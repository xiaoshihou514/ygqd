<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useFollowedAuthors } from '@/composables/useFollowedAuthors'
import { fetchComicMetadata, fetchViewHistory, searchComics } from '@/services/api'
import { datedJsonFilename, exportJson, type ExportResult } from '@/utils/exportJson'
import type { ComicItem, FollowedAuthor, ViewHistoryEntry } from '@/types'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const router = useRouter()
const { followed, loading: loadingFollows, load, unfollow } = useFollowedAuthors()

const expandedAuthor = ref<string | null>(null)
const authorWorks = ref<Record<string, { items: ComicItem[]; loading: boolean }>>({})
const newByAuthor = ref<Record<string, ComicItem[]>>({})
const syncing = ref(false)
const syncProgress = ref('')
const syncError = ref('')
const exporting = ref(false)
const exportStatus = ref('')

const newItems = computed(() => {
  const seen = new Set<string>()
  return followed.value.flatMap((fa) => newByAuthor.value[fa.author] ?? [])
    .filter((item) => {
      const key = `${item.categoryId}:${item.id}`
      if (seen.has(key)) return false
      seen.add(key)
      return true
    })
})

onMounted(async () => {
  await load()
  if (followed.value.length > 0) {
    await syncNewWorks()
  }
})

function siteDate(ms: number): string {
  return new Date(ms + 8 * 60 * 60 * 1000).toISOString().slice(0, 10)
}

function sameAuthor(actual: string, expected: string): boolean {
  const normalized = expected.trim().toLocaleLowerCase()
  return actual.split(',').some((name) => name.trim().toLocaleLowerCase() === normalized)
}

function baselineFor(follow: FollowedAuthor, history: ViewHistoryEntry[]): string {
  const lastRead = history
    .filter((entry) => sameAuthor(entry.author, follow.author))
    .reduce((latest, entry) => Math.max(latest, entry.viewedAt), 0)
  return siteDate(Math.max(lastRead, follow.followedAt))
}

async function fetchAllAuthorWorks(author: string): Promise<ComicItem[]> {
  const params = { keyword: author, classid: 9, show: 'title,text,keyboard,ftitle', tempid: '1' }
  const cacheBuster = String(Date.now())
  const first = await searchComics(params, 0, cacheBuster)
  const all = [...first.items]
  const totalPages = Math.min(Math.max(first.pagination.total, 1), 50)
  for (let page = 1; page < totalPages; page += 3) {
    const pages = Array.from(
      { length: Math.min(3, totalPages - page) },
      (_, offset) => page + offset,
    )
    const results = await Promise.all(pages.map((value) => searchComics(params, value, cacheBuster)))
    all.push(...results.flatMap((result) => result.items))
  }
  const seen = new Set<string>()
  return all.filter((item) => {
    const key = `${item.categoryId}:${item.id}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

async function findNewWorks(
  follow: FollowedAuthor,
  history: ViewHistoryEntry[],
  items: ComicItem[],
): Promise<ComicItem[]> {
  const baseline = baselineFor(follow, history)
  const viewed = new Set(history.map((entry) => `${entry.categoryId}:${entry.comicId}`))
  const found: ComicItem[] = []
  for (let index = 0; index < items.length; index += 4) {
    const batch = items.slice(index, index + 4)
    const metadata = await Promise.all(batch.map((item) =>
      fetchComicMetadata(item.categoryId, item.id).catch(() => null),
    ))
    metadata.forEach((value, offset) => {
      const item = batch[offset]
      if (!item || !value?.publishedAt) return
      if (!sameAuthor(value.author, follow.author)) return
      if (viewed.has(`${item.categoryId}:${item.id}`)) return
      if (value.publishedAt > baseline) found.push(item)
    })
  }
  return found
}

const CONCURRENCY = 3

async function syncNewWorks() {
  if (syncing.value) return
  syncing.value = true
  syncError.value = ''
  try {
    const history = await fetchViewHistory(10000)
    const queue = [...followed.value]
    const total = queue.length
    let done = 0
    const workerCount = Math.min(CONCURRENCY, total)
    const workers = Array.from({ length: workerCount }, async () => {
      while (queue.length > 0) {
        const follow = queue.shift()
        if (!follow) continue
        try {
          const items = await fetchAllAuthorWorks(follow.author)
          authorWorks.value[follow.author] = { items, loading: false }
          newByAuthor.value[follow.author] = await findNewWorks(follow, history, items)
        } catch {
          newByAuthor.value[follow.author] = []
          syncError.value = '部分作者检查失败，可稍后重试'
        } finally {
          done++
          syncProgress.value = `正在检查 ${total} 位作者（${done}/${total}）`
        }
      }
    })
    await Promise.all(workers)
  } catch {
    syncError.value = '检查新作失败，请确认网络后重试'
  } finally {
    syncing.value = false
    syncProgress.value = ''
  }
}

async function toggleAuthor(author: string) {
  if (expandedAuthor.value === author) {
    expandedAuthor.value = null
    return
  }
  expandedAuthor.value = author
  if (!authorWorks.value[author]) {
    authorWorks.value[author] = { items: [], loading: true }
    try {
      const result = await searchComics({ keyword: author, classid: 9, show: 'title,text,keyboard,ftitle', tempid: '1' })
      authorWorks.value[author] = { items: result.items, loading: false }
    } catch {
      authorWorks.value[author] = { items: [], loading: false }
    }
  }
}

function goToAuthor(author: string) {
  router.push({ name: 'search', query: { keyword: author, show: 'title,text,keyboard,ftitle' } })
}

function formatTime(ms: number): string {
  const diff = Date.now() - ms
  const mins = Math.floor(diff / 60000)
  if (mins < 60) return `${mins} 分钟前`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours} 小时前`
  const days = Math.floor(hours / 24)
  return `${days} 天前`
}

const EXPORT_MESSAGES: Record<ExportResult, string> = {
  shared: '已发送关注作者备份',
  downloaded: '关注作者备份已下载',
  copied: '无法保存文件，备份 JSON 已复制',
  cancelled: '',
}

async function handleExport() {
  if (exporting.value) return
  exporting.value = true
  exportStatus.value = ''
  try {
    const result = await exportJson({
      filename: datedJsonFilename('ygqd-followed-authors'),
      title: 'ygqd 关注作者备份',
      data: {
        format: 'ygqd.followed-authors',
        version: 1,
        exportedAt: new Date().toISOString(),
        followedAuthors: followed.value,
      },
    })
    exportStatus.value = EXPORT_MESSAGES[result]
  } catch {
    exportStatus.value = '导出失败，请稍后重试'
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div class="following-page">
    <div class="container">
      <div class="page-heading">
        <h2 class="page-title">关注作者</h2>
        <button class="export-btn" :disabled="exporting" @click="handleExport">
          {{ exporting ? '导出中' : '导出' }}
        </button>
      </div>
      <p v-if="exportStatus" class="export-status" aria-live="polite">{{ exportStatus }}</p>

      <LoadingSpinner v-if="loadingFollows" message="加载中..." />

      <EmptyState
        v-else-if="followed.length === 0"
        title="还没有关注任何作者"
        message="在漫画详情页点击作者旁的心形图标即可关注"
      />

      <template v-else>
        <section class="update-panel" :class="{ hasUpdates: newItems.length > 0 }">
          <div class="update-copy">
            <span class="update-kicker">关注动态</span>
            <strong v-if="syncing">{{ syncProgress || '正在检查新作' }}</strong>
            <strong v-else-if="newItems.length">发现 {{ newItems.length }} 部确定的新作</strong>
            <strong v-else>暂时没有确定的新作</strong>
            <span v-if="!syncing" class="update-detail">以最后阅读日期为准，同日发布的作品不会误报</span>
          </div>
          <button class="refresh-btn" :disabled="syncing" @click="syncNewWorks">
            {{ syncing ? '检查中' : '重新检查' }}
          </button>
        </section>

        <p v-if="syncError" class="sync-error">{{ syncError }}</p>

        <section v-if="newItems.length" class="new-works">
          <h3 class="section-title">新作</h3>
          <ComicGrid :items="newItems" />
        </section>

      <div class="following-list">
        <div
          v-for="fa in followed"
          :key="fa.author"
          class="author-card"
          :class="{ expanded: expandedAuthor === fa.author }"
        >
          <div class="author-header" @click="toggleAuthor(fa.author)">
            <span class="author-name">{{ fa.author }}</span>
            <span v-if="newByAuthor[fa.author]?.length" class="new-count">
              {{ newByAuthor[fa.author]!.length }} 部新作
            </span>
            <span class="author-meta">关注于 {{ formatTime(fa.followedAt) }}</span>
            <div class="author-actions" @click.stop>
              <button
                class="author-action-btn"
                :title="`搜索 ${fa.author}`"
                @click="goToAuthor(fa.author)"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <circle cx="11" cy="11" r="8" />
                  <path d="M21 21l-4.35-4.35" />
                </svg>
              </button>
              <button
                class="author-action-btn author-action-danger"
                :title="`取消关注 ${fa.author}`"
                @click="unfollow(fa.author)"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" fill="currentColor" />
                </svg>
              </button>
            </div>
          </div>

          <div v-if="expandedAuthor === fa.author" class="author-works">
            <LoadingSpinner
              v-if="authorWorks[fa.author]?.loading"
              message="加载作品..."
            />
            <ComicGrid
              v-else-if="authorWorks[fa.author]?.items.length"
              :items="authorWorks[fa.author]!.items"
            />
            <EmptyState
              v-else
              title="未找到作品"
              :message="`未能搜索到 ${fa.author} 的作品`"
            />
          </div>
        </div>
      </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.following-page {
  padding-bottom: var(--spacing-xl);
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
}

.page-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-lg);
  padding-top: var(--spacing-lg);
}

.export-btn {
  flex: none;
  min-height: 36px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  background: var(--color-card-bg);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
}

.export-btn:disabled {
  opacity: 0.55;
}

.export-status {
  margin: calc(var(--spacing-md) * -1) 0 var(--spacing-lg);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.following-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-sm);
}

.update-panel {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-lg);
  margin-bottom: var(--spacing-lg);
  padding: var(--spacing-lg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  background: var(--color-card-bg);
}

.update-panel.hasUpdates {
  border-color: color-mix(in srgb, var(--color-primary) 28%, var(--color-hairline));
  background: color-mix(in srgb, var(--color-primary) 5%, var(--color-card-bg));
}

.update-copy {
  display: flex;
  flex-direction: column;
  gap: 3px;
  color: var(--color-text-primary);
}

.update-kicker {
  color: var(--color-primary);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
}

.update-detail {
  color: var(--color-text-muted);
  font-size: var(--font-size-fine-print);
}

.refresh-btn {
  flex: none;
  padding: 8px 14px;
  border-radius: var(--radius-pill);
  color: var(--color-on-primary);
  background: var(--color-primary);
  font-size: var(--font-size-caption);
}

.refresh-btn:disabled {
  opacity: 0.55;
}

.sync-error {
  margin: calc(var(--spacing-md) * -1) 0 var(--spacing-lg);
  color: var(--color-error);
  font-size: var(--font-size-caption);
}

.new-works {
  margin-bottom: var(--spacing-xl);
}

.section-title {
  margin-bottom: var(--spacing-md);
  color: var(--color-text-primary);
  font-size: var(--font-size-tagline);
}

.new-count {
  padding: 3px 8px;
  border-radius: var(--radius-pill);
  color: var(--color-primary);
  background: color-mix(in srgb, var(--color-primary) 10%, transparent);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
}

@media (max-width: 560px) {
  .update-panel {
    align-items: flex-start;
    padding: var(--spacing-md);
  }

  .author-header {
    gap: var(--spacing-xs);
    flex-wrap: wrap;
  }

  .author-meta {
    order: 3;
    width: 100%;
  }
}

.author-card {
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.author-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: var(--spacing-md) var(--spacing-lg);
  cursor: pointer;
  transition: background 0.15s;
}

.author-header:hover {
  background: var(--color-primary-focus);
}

.author-name {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  flex: 1;
}

.author-meta {
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.author-actions {
  display: flex;
  gap: 4px;
}

.author-action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  transition: all 0.15s;
}

.author-action-btn:hover {
  background: rgba(0, 0, 0, 0.08);
  color: var(--color-text-primary);
}

.author-action-danger:hover {
  color: var(--color-error);
}

.author-works {
  border-top: 1px solid var(--color-hairline);
  padding: var(--spacing-md);
}
</style>
