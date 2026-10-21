<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { fetchVideoList, searchVideos } from '@/services/api'
import type { VideoCard } from '@/types'
import ProxyImage from '@/components/ProxyImage.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const router = useRouter()
const items = ref<VideoCard[]>([])
const loading = ref(true)
const loadingMore = ref(false)
const error = ref('')
const keyword = ref('')
const activeKeyword = ref('')
const page = ref(1)
const hasNext = ref(true)
let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

async function load(reset: boolean) {
  if (reset) {
    loading.value = true
    error.value = ''
    page.value = 1
  } else {
    if (loadingMore.value || !hasNext.value || loading.value) return
    loadingMore.value = true
  }
  try {
    const result = activeKeyword.value
      ? await searchVideos(activeKeyword.value, page.value)
      : await fetchVideoList(page.value)
    if (reset) {
      items.value = result.items
    } else {
      const seen = new Set(items.value.map((v) => v.code))
      items.value.push(...result.items.filter((v) => !seen.has(v.code)))
    }
    hasNext.value = result.pagination.hasNext
    page.value += 1
  } catch (e) {
    error.value = (e as Error).message || '加载失败'
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

function submitSearch() {
  activeKeyword.value = keyword.value.trim().toLowerCase()
  void load(true)
}

function clearSearch() {
  keyword.value = ''
  activeKeyword.value = ''
  void load(true)
}

function setupObserver() {
  sentinel = document.querySelector('.scroll-sentinel')
  if (!sentinel) return
  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0]?.isIntersecting) void load(false)
    },
    { rootMargin: '400px' },
  )
  observer.observe(sentinel)
}

onMounted(async () => {
  await load(true)
  setupObserver()
})

onUnmounted(() => {
  observer?.disconnect()
})
</script>

<template>
  <div class="video-browser">
      <div class="video-heading">
        <form class="video-search" @submit.prevent="submitSearch">
          <input
            v-model="keyword"
            type="search"
            class="video-search-input"
            placeholder="输入番号或关键词,如 ssis-406"
            autocomplete="off"
          >
          <button type="submit" class="video-search-btn">搜索</button>
          <button v-if="activeKeyword" type="button" class="video-search-btn" @click="clearSearch">
            清除
          </button>
        </form>
      </div>

      <LoadingSpinner v-if="loading" message="加载中..." />
      <EmptyState
        v-else-if="error"
        :title="activeKeyword ? '搜索失败' : '加载失败'"
        :message="error"
      />
      <EmptyState
        v-else-if="!items.length"
        :title="activeKeyword ? '没有找到结果' : '暂无内容'"
        :message="activeKeyword ? `没有与「${activeKeyword}」匹配的视频,换个番号试试` : '稍后再来看看'"
      />
      <div v-else class="video-grid">
        <button
          v-for="card in items"
          :key="card.code"
          class="video-card"
          @click="router.push(`/videos/${card.code}`)"
        >
          <div class="video-thumb-wrap">
            <ProxyImage :src="card.thumbnail" source="video" class="video-thumb" loading="lazy" />
            <span v-if="card.views" class="video-views">▶ {{ card.views }}</span>
          </div>
          <div class="video-card-body">
            <span class="video-code-row">
              <span class="video-code">{{ card.code.replace('-chinese-subtitle', '').toUpperCase() }}</span>
              <span v-if="card.chinese" class="video-cs-badge">中字</span>
            </span>
            <span class="video-card-title">{{ card.title || card.code }}</span>
          </div>
        </button>
      </div>

      <div v-if="loadingMore" class="loading-more">
        <LoadingSpinner message="正在加载更多..." />
      </div>
      <div v-if="!loading && hasNext && items.length" class="scroll-sentinel" />
      <div v-else-if="!hasNext && items.length" class="no-more">
        <span>没有更多了</span>
      </div>
  </div>
</template>

<style scoped>
.video-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-md);
  flex-wrap: wrap;
  margin-bottom: var(--spacing-md);
}

.video-search {
  display: flex;
  gap: var(--spacing-xs);
  flex: 1;
  max-width: 420px;
}

.video-search-input {
  flex: 1;
  min-width: 0;
  padding: 8px 14px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
}

.video-search-btn {
  padding: 8px 16px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.video-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-md);
}

.video-card {
  display: flex;
  flex-direction: column;
  padding: 0;
  border: none;
  background: var(--color-card-bg);
  border-radius: var(--radius-md);
  overflow: hidden;
  cursor: pointer;
  text-align: left;
}

.video-thumb-wrap {
  position: relative;
  aspect-ratio: 16 / 9;
  background: var(--color-divider-soft);
}

.video-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.video-views {
  position: absolute;
  right: 6px;
  bottom: 6px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.65);
  color: #fff;
  font-size: 11px;
}

.video-card-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
}

.video-code-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.video-code {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-primary);
}

.video-cs-badge {
  padding: 0 5px;
  border-radius: 4px;
  background: #e8590c;
  color: #fff;
  font-size: 10px;
  flex-shrink: 0;
}

.video-card-title {
  font-size: 12px;
  color: var(--color-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.loading-more,
.no-more {
  display: flex;
  justify-content: center;
  padding: var(--spacing-lg) 0;
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
}
</style>
