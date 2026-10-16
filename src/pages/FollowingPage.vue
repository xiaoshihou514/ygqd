<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { useFollowedAuthors } from '@/composables/useFollowedAuthors'
import { searchComics } from '@/services/api'
import { datedJsonFilename, exportJson, type ExportResult } from '@/utils/exportJson'
import type { ComicItem, FollowedAuthor } from '@/types'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const router = useRouter()
const { followed, loading: loadingFollows, load, unfollow } = useFollowedAuthors()

const tabs = [
  { key: 'authors', label: '作者' },
  { key: 'works', label: '作品' },
] as const
type FollowingTab = (typeof tabs)[number]['key']

const activeTab = ref<FollowingTab>('authors')
const tabScroll: Record<FollowingTab, number> = { authors: 0, works: 0 }

interface SourcePageState {
  page: number
  hasNext: boolean
}

interface AuthorWorksState {
  items: ComicItem[]
  niacg: SourcePageState
  bika: SourcePageState
  loading: boolean
}

const authorWorks = ref<Record<string, AuthorWorksState>>({})
const syncing = ref(false)
const syncProgress = ref('')
const syncError = ref('')
const bikaHint = ref('')
const loadingMore = ref(false)
const exporting = ref(false)
const exportStatus = ref('')
let syncCacheBuster = ''
let nextAuthorIndex = 0
let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

const allWorks = computed<ComicItem[]>(() => {
  const seen = new Set<string>()
  const merged: ComicItem[] = []
  for (const fa of followed.value) {
    for (const item of authorWorks.value[fa.author]?.items ?? []) {
      const key = `${item.source}:${item.id}`
      if (seen.has(key)) continue
      seen.add(key)
      merged.push(item)
    }
  }
  return merged.sort((a, b) => (Number(b.id) || 0) - (Number(a.id) || 0))
})

const hasMoreWorks = computed(() =>
  followed.value.some((fa) => {
    const state = authorWorks.value[fa.author]
    return state && (state.niacg.hasNext || state.bika.hasNext)
  }),
)

onMounted(async () => {
  await load()
  if (followed.value.length > 0) {
    await loadAllAuthors()
    await nextTick()
    setupObserver()
  }
})

async function fetchAuthorPage(
  follow: FollowedAuthor,
  source: 'niacg' | 'bika',
  page: number,
): Promise<void> {
  const result = await searchComics(
    source,
    {
      source,
      keyword: follow.author,
      classid: 9,
      show: source === 'bika' ? 'writer' : 'title,text,keyboard,ftitle',
      tempid: '1',
    },
    page,
    syncCacheBuster,
  )
  const state = authorWorks.value[follow.author]
  if (!state) return
  const seen = new Set(state.items.map((item) => `${item.source}:${item.id}`))
  state.items.push(...result.items.filter((item) => {
    const key = `${item.source}:${item.id}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  }))
  state[source] = { page, hasNext: result.pagination.hasNext }
}

const CONCURRENCY = 3

async function loadAllAuthors() {
  if (syncing.value || followed.value.length === 0) return
  syncing.value = true
  syncError.value = ''
  bikaHint.value = ''
  syncCacheBuster = String(Date.now())
  authorWorks.value = {}
  nextAuthorIndex = 0

  const queue = [...followed.value]
  const total = queue.length
  let done = 0
  const workerCount = Math.min(CONCURRENCY, total)
  const workers = Array.from({ length: workerCount }, async () => {
    while (queue.length > 0) {
      const follow = queue.shift()
      if (!follow) continue
      authorWorks.value[follow.author] = {
        items: [],
        niacg: { page: -1, hasNext: true },
        bika: { page: -1, hasNext: true },
        loading: true,
      }
      const sourceFetches = (['niacg', 'bika'] as const).map((source) => ({
        source,
        request: fetchAuthorPage(follow, source, 0),
      }))
      const settled = await Promise.allSettled(sourceFetches.map((fetch) => fetch.request))
      const state = authorWorks.value[follow.author]!
      state.loading = false
      sourceFetches.forEach((fetch, index) => {
        const result = settled[index]!
        if (result.status === 'fulfilled') return
        state[fetch.source].hasNext = false
        if (fetch.source === 'niacg') {
          syncError.value = '部分作者作品加载失败，可稍后刷新重试'
        } else {
          const reason = String(result.reason)
          if (reason.includes('登录') && !bikaHint.value) {
            bikaHint.value = '哔咔源未登录，可在设置中登录后同时查看哔咔作品'
          }
        }
      })
      done++
      syncProgress.value = `正在加载 ${total} 位作者的作品（${done}/${total}）`
    }
  })
  await Promise.all(workers)
  syncing.value = false
  syncProgress.value = ''
  await nextTick()
  setupObserver()
}

async function loadNextPage() {
  if (loadingMore.value || syncing.value || followed.value.length === 0) return

  const start = nextAuthorIndex % followed.value.length
  let selected: { follow: FollowedAuthor; source: 'niacg' | 'bika'; page: number } | null = null
  for (let offset = 0; offset < followed.value.length; offset += 1) {
    const index = (start + offset) % followed.value.length
    const follow = followed.value[index]
    const state = follow ? authorWorks.value[follow.author] : undefined
    if (follow && state && !state.loading) {
      if (state.niacg.hasNext) {
        selected = { follow, source: 'niacg', page: state.niacg.page + 1 }
      } else if (state.bika.hasNext) {
        selected = { follow, source: 'bika', page: state.bika.page + 1 }
      }
      if (selected) {
        nextAuthorIndex = (index + 1) % followed.value.length
        break
      }
    }
  }
  if (!selected) return

  loadingMore.value = true
  authorWorks.value[selected.follow.author]!.loading = true
  try {
    await fetchAuthorPage(selected.follow, selected.source, selected.page)
  } catch {
    const state = authorWorks.value[selected.follow.author]
    if (state) {
      state[selected.source].hasNext = false
      state.loading = false
    }
    syncError.value = '加载更多作品失败，请重试'
  } finally {
    const state = authorWorks.value[selected.follow.author]
    if (state) state.loading = false
    loadingMore.value = false
  }
}

async function switchTab(tab: FollowingTab) {
  if (tab === activeTab.value) return
  tabScroll[activeTab.value] = window.scrollY
  activeTab.value = tab
  await nextTick()
  window.scrollTo(0, tabScroll[tab])
  setupObserver()
}

function setupObserver() {
  observer?.disconnect()
  sentinel = document.querySelector('.scroll-sentinel')
  if (!sentinel) return

  observer = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting) {
      loadNextPage()
    }
  }, { rootMargin: '200px' })

  observer.observe(sentinel)
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

onUnmounted(() => {
  observer?.disconnect()
  observer = null
})
</script>

<template>
  <div class="following-page">
    <div class="container">
      <div class="page-heading">
        <h2 class="page-title">关注</h2>
        <div class="heading-actions">
          <button
            class="action-btn"
            :disabled="syncing || loadingFollows"
            @click="loadAllAuthors"
          >
            刷新
          </button>
          <button class="action-btn" :disabled="exporting" @click="handleExport">
            {{ exporting ? '导出中' : '导出' }}
          </button>
        </div>
      </div>
      <p v-if="exportStatus" class="export-status" aria-live="polite">{{ exportStatus }}</p>

      <LoadingSpinner v-if="loadingFollows" message="加载中..." />

      <EmptyState
        v-else-if="followed.length === 0"
        title="还没有关注任何作者"
        message="在漫画详情页点击作者旁的心形图标即可关注"
      />

      <template v-else>
        <div class="content-tabs" role="tablist" aria-label="关注视图">
          <button
            v-for="tab in tabs"
            :key="tab.key"
            class="content-tab"
            :class="{ active: activeTab === tab.key }"
            role="tab"
            :aria-selected="activeTab === tab.key"
            @click="switchTab(tab.key)"
          >
            {{ tab.label }}
          </button>
          <span v-if="activeTab === 'works'" class="works-count">共 {{ allWorks.length }} 部</span>
        </div>

        <p v-if="syncing && syncProgress" class="sync-progress">{{ syncProgress }}</p>
        <p v-if="syncError && !syncing" class="sync-error">{{ syncError }}</p>
        <p v-if="bikaHint && !syncing" class="sync-error">{{ bikaHint }}</p>

        <template v-if="activeTab === 'authors'">
          <div class="following-list">
            <div
              v-for="fa in followed"
              :key="fa.author"
              class="author-card"
            >
              <div class="author-header">
                <span class="author-name">{{ fa.author }}</span>
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

              <div class="author-works">
                <LoadingSpinner
                  v-if="!authorWorks[fa.author]?.items.length && (syncing || authorWorks[fa.author]?.loading)"
                  message="加载作品..."
                />
                <ComicGrid
                  v-if="authorWorks[fa.author]?.items.length"
                  :items="authorWorks[fa.author]!.items"
                />
                <EmptyState
                  v-if="!syncing && !authorWorks[fa.author]?.loading && !authorWorks[fa.author]?.items.length"
                  title="未找到作品"
                  :message="`未能搜索到 ${fa.author} 的作品`"
                />
              </div>
            </div>
          </div>
        </template>

        <template v-else>
          <ComicGrid v-if="allWorks.length" :items="allWorks" />
          <LoadingSpinner v-else-if="syncing" message="正在加载作品..." />
          <EmptyState v-else title="暂无作品" message="未能获取到关注作者的作品" />
        </template>

        <div v-if="loadingMore" class="loading-more">
          <LoadingSpinner message="正在加载更多..." />
        </div>

        <div v-if="!syncing && !loadingMore && !hasMoreWorks && allWorks.length" class="no-more">
          <span>已加载全部作品</span>
        </div>

        <div v-if="hasMoreWorks && !syncing" class="scroll-sentinel" />
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

.heading-actions {
  display: flex;
  gap: var(--spacing-xs);
}

.action-btn {
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

.action-btn:disabled {
  opacity: 0.55;
}

.export-status {
  margin: calc(var(--spacing-md) * -1) 0 var(--spacing-lg);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.content-tabs {
  display: inline-flex;
  align-items: center;
  padding: 3px;
  margin-bottom: var(--spacing-lg);
  border-radius: var(--radius-pill);
  background: var(--color-divider-soft);
}

.content-tab {
  min-width: 66px;
  padding: 7px 16px;
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  background: transparent;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  transition: color 0.18s, background 0.18s, box-shadow 0.18s;
}

.content-tab.active {
  color: var(--color-text-primary);
  background: var(--color-card-bg);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.1);
}

.content-tab:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

.works-count {
  padding-right: var(--spacing-md);
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.sync-progress {
  margin: calc(var(--spacing-md) * -1) 0 var(--spacing-lg);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.sync-error {
  margin: calc(var(--spacing-md) * -1) 0 var(--spacing-lg);
  color: var(--color-error);
  font-size: var(--font-size-caption);
}

.following-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-sm);
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
  cursor: default;
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

.loading-more {
  padding: var(--spacing-lg) 0;
}

.scroll-sentinel {
  height: 1px;
}

.no-more {
  padding: var(--spacing-lg) 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
  text-align: center;
}

@media (max-width: 560px) {
  .author-header {
    gap: var(--spacing-xs);
    flex-wrap: wrap;
  }

  .author-meta {
    order: 3;
    width: 100%;
  }
}
</style>
