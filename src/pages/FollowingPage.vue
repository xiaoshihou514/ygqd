<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useFollowedAuthors } from '@/composables/useFollowedAuthors'
import { searchComics } from '@/services/api'
import type { ComicItem } from '@/types'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const router = useRouter()
const { followed, loading: loadingFollows, load, unfollow } = useFollowedAuthors()

const expandedAuthor = ref<string | null>(null)
const authorWorks = ref<Record<string, { items: ComicItem[]; loading: boolean }>>({})

onMounted(async () => {
  await load()
})

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
</script>

<template>
  <div class="following-page">
    <div class="container">
      <h2 class="page-title">关注作者</h2>

      <LoadingSpinner v-if="loadingFollows" message="加载中..." />

      <EmptyState
        v-else-if="followed.length === 0"
        title="还没有关注任何作者"
        message="在漫画详情页点击作者旁的心形图标即可关注"
      />

      <div v-else class="following-list">
        <div
          v-for="fa in followed"
          :key="fa.author"
          class="author-card"
          :class="{ expanded: expandedAuthor === fa.author }"
        >
          <div class="author-header" @click="toggleAuthor(fa.author)">
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
  margin-bottom: var(--spacing-lg);
  padding-top: var(--spacing-lg);
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
