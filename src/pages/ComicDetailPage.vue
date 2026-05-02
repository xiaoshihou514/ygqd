<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { fetchComicDetail } from '@/services/api'
import type { ComicDetail } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const PAGE_SIZE = 10

const route = useRoute()
const router = useRouter()

const detail = ref<ComicDetail | null>(null)
const loading = ref(true)
const error = ref('')
const visibleCount = ref(PAGE_SIZE)

const visibleImages = computed(() => {
  if (!detail.value) return []
  return detail.value.images.slice(0, visibleCount.value)
})

const hasMore = computed(() => {
  if (!detail.value) return false
  return visibleCount.value < detail.value.images.length
})

function loadMore() {
  if (!detail.value || !hasMore.value) return
  visibleCount.value = Math.min(visibleCount.value + PAGE_SIZE, detail.value.images.length)
  nextTick(() => {
    setupObserver()
  })
}

let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

function setupObserver() {
  if (observer) {
    observer.disconnect()
  }
  sentinel = document.querySelector('.scroll-sentinel')
  if (!sentinel || !hasMore.value) return

  observer = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting) {
      loadMore()
    }
  }, { rootMargin: '200px' })

  observer.observe(sentinel)
}

onMounted(async () => {
  const categoryId = Number(route.params.categoryId)
  const id = route.params.id as string

  try {
    detail.value = await fetchComicDetail(categoryId, id)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    loading.value = false
    await nextTick()
    setupObserver()
  }
})

onUnmounted(() => {
  if (observer) {
    observer.disconnect()
    observer = null
  }
})

function goBack() {
  router.back()
}

function searchByScope(keyword: string, show: string) {
  router.push({ name: 'search', query: { keyword, show } })
}
</script>

<script lang="ts">
import { computed, nextTick } from 'vue'
export default { inheritAttrs: false }
</script>

<template>
  <div class="detail-page">
    <div class="container">
      <button class="back-btn" @click="goBack">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M19 12H5M12 19l-7-7 7-7" />
        </svg>
        返回
      </button>

      <div v-if="loading" class="detail-loading">
        <LoadingSpinner message="正在加载..." />
      </div>

      <div v-else-if="error" class="detail-error">
        <EmptyState title="加载失败" :message="error" />
      </div>

      <template v-else-if="detail">
        <div class="detail-header">
          <div class="cover-col">
            <div class="cover-wrap">
              <img
                v-if="detail.thumbnail"
                :src="detail.thumbnail"
                :alt="detail.title"
                referrerpolicy="no-referrer"
              />
              <div v-else class="cover-placeholder">
                <span>{{ detail.title }}</span>
              </div>
            </div>
          </div>
          <div class="info-col">
            <h1 class="detail-title">{{ detail.title }}</h1>
            <div class="detail-meta">
              <span class="meta-category">{{ detail.category }}</span>
              <span class="meta-likes">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="var(--color-like)">
                  <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
                </svg>
                {{ detail.likes }}
              </span>
            </div>
            <div v-if="detail.author" class="detail-tag-row">
              <span class="tag-label">作者</span>
              <div class="detail-tags">
                <button class="detail-tag" @click="searchByScope(detail.author)">
                  {{ detail.author }}
                </button>
              </div>
            </div>
            <div v-if="detail.works.length > 0" class="detail-tag-row">
              <span class="tag-label">作品</span>
              <div class="detail-tags">
                <button
                  v-for="work in detail.works"
                  :key="work"
                  class="detail-tag"
                  @click="searchByScope(work, 'title,text')"
                >
                  {{ work }}
                </button>
              </div>
            </div>
            <div v-if="detail.characters.length > 0" class="detail-tag-row">
              <span class="tag-label">人物</span>
              <div class="detail-tags">
                <button
                  v-for="char in detail.characters"
                  :key="char"
                  class="detail-tag"
                  @click="searchByScope(char, 'title,text,keyboard,ftitle')"
                >
                  {{ char }}
                </button>
              </div>
            </div>
            <div v-if="detail.tags.length > 0" class="detail-tag-row">
              <span class="tag-label">标签</span>
              <div class="detail-tags">
                <button
                  v-for="tag in detail.tags"
                  :key="tag"
                  class="detail-tag"
                  @click="searchByScope(tag, 'tags')"
                >
                  {{ tag }}
                </button>
              </div>
            </div>
            <div class="detail-count">
              共 {{ detail.images.length }} 张图片
            </div>
          </div>
        </div>

        <div v-if="detail.images.length > 0" class="detail-gallery">
          <img
            v-for="(img, i) in visibleImages"
            :key="i"
            :src="img"
            :alt="`${detail.title} - 第${i + 1}页`"
            loading="lazy"
            referrerpolicy="no-referrer"
            class="gallery-img"
          />

          <div v-if="hasMore" class="scroll-sentinel" />
          <div v-if="!hasMore && detail.images.length > PAGE_SIZE" class="no-more">
            <span>已加载全部 {{ detail.images.length }} 张图片</span>
          </div>
        </div>

        <div v-else class="detail-empty">
          <EmptyState title="暂无图片" message="该漫画暂无图片内容" />
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.detail-page {
  padding-bottom: var(--spacing-xl);
}

.back-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: var(--spacing-xs) var(--spacing-md);
  margin-bottom: var(--spacing-md);
  color: var(--color-text-secondary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  background: var(--color-card-bg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  transition: all 0.2s;
}

.back-btn:hover {
  color: var(--color-primary);
  border-color: var(--color-primary);
}

.detail-loading {
  padding-top: var(--spacing-xl);
}

.detail-error {
  padding-top: var(--spacing-xl);
}

.detail-header {
  display: flex;
  gap: var(--spacing-lg);
  margin-bottom: var(--spacing-xl);
  padding-bottom: var(--spacing-lg);
  border-bottom: 1px solid var(--color-hairline);
}

.cover-col {
  flex-shrink: 0;
  width: 240px;
}

.cover-wrap {
  position: relative;
  width: 100%;
  padding-top: 140%;
  overflow: hidden;
  border-radius: var(--radius-lg);
  background: var(--color-divider-soft);
}

.cover-wrap img {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cover-placeholder {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-sm);
  color: var(--color-text-muted);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  text-align: center;
}

.info-col {
  flex: 1;
  min-width: 0;
}

.detail-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  line-height: 1.3;
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-sm);
}

.detail-meta {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-sm);
}

.meta-category {
  padding: 2px 10px;
  background: var(--color-primary);
  color: var(--color-on-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  border-radius: var(--radius-pill);
}

.meta-likes {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
}

.detail-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
}

.detail-tag-row {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xxs);
}

.tag-label {
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-muted);
  min-width: 36px;
}

.detail-tag {
  padding: 2px 10px;
  background: var(--color-tag-bg);
  color: var(--color-tag-text);
  font-size: var(--font-size-caption);
  border-radius: var(--radius-sm);
  border: none;
  transition: all 0.2s;
}

.detail-tag:hover {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.detail-count {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.detail-gallery {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-sm);
}

.gallery-img {
  max-width: 100%;
  max-height: 90vh;
  border-radius: var(--radius-md);
}

.scroll-sentinel {
  height: 1px;
}

.no-more {
  text-align: center;
  padding: var(--spacing-lg) 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.detail-empty {
  padding-top: var(--spacing-lg);
}

@media (max-width: 768px) {
  .detail-header {
    flex-direction: column;
  }

  .cover-col {
    width: 100%;
    max-width: 320px;
  }

  .detail-title {
    font-size: var(--font-size-tagline);
  }
}
</style>
