<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { novelProgress, novelRead, novelSetProgress, novelShelf } from '@/services/api'
import type { NovelPage, NovelSeries } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const route = useRoute()
const router = useRouter()

const series = ref<NovelSeries | null>(null)
const page = ref<NovelPage | null>(null)
const chapterIndex = ref(0)
const articlePage = ref(1)
const loading = ref(true)
const pageLoading = ref(false)
const error = ref('')
const showChapters = ref(false)

const chapters = computed(() => series.value?.chapters ?? [])
const currentChapter = computed(() => chapters.value[chapterIndex.value] ?? null)
const hasNextPage = computed(() => !!page.value && articlePage.value < page.value.totalPages)
const hasPrevPage = computed(() => articlePage.value > 1)
const hasNextChapter = computed(() => chapterIndex.value < chapters.value.length - 1)
const hasPrevChapter = computed(() => chapterIndex.value > 0)

async function loadPage(silent = false) {
  if (!currentChapter.value) return
  if (!silent) pageLoading.value = true
  error.value = ''
  try {
    page.value = await novelRead(currentChapter.value.id, articlePage.value)
    if (series.value) {
      void novelSetProgress(
        series.value.id,
        currentChapter.value.order,
        articlePage.value,
      )
    }
  } catch (e) {
    error.value = (e as Error).message || '加载失败'
  } finally {
    pageLoading.value = false
    loading.value = false
  }
}

function goPage(delta: number) {
  if (!hasPrevPage.value && delta < 0) return
  if (!hasNextPage.value && delta > 0) {
    // 页到头 -> 下一章第 1 页
    if (hasNextChapter.value) {
      chapterIndex.value += 1
      articlePage.value = 1
      void loadPage()
    }
    return
  }
  articlePage.value += delta
  void loadPage()
}

function goChapter(index: number) {
  chapterIndex.value = index
  articlePage.value = 1
  showChapters.value = false
  void loadPage()
}

function jumpToProgress() {
  void (async () => {
    if (!series.value) return
    try {
      const p = await novelProgress(series.value.id)
      if (p) {
        const [order, pg] = p
        const idx = chapters.value.findIndex((c) => c.order === order)
        if (idx >= 0) {
          chapterIndex.value = idx
          articlePage.value = pg
        }
      }
    } catch { /* 忽略 */ }
    await loadPage()
  })()
}

onMounted(async () => {
  const id = String(route.params.id || '')
  try {
    const shelf = await novelShelf()
    series.value = shelf.find((s) => s.id === id) ?? null
    if (!series.value) {
      error.value = '书架中找不到该系列,请重新添加'
      loading.value = false
      return
    }
    jumpToProgress()
  } catch (e) {
    error.value = (e as Error).message || '加载失败'
    loading.value = false
  }
})
</script>

<template>
  <div class="reader-page">
    <div class="reader-header">
      <button class="hdr-btn" @click="router.push('/novels')">‹ 书架</button>
      <div class="hdr-center">
        <span class="hdr-title">{{ currentChapter ? `#${currentChapter.order} ${currentChapter.title || currentChapter.id}` : (series?.title ?? '') }}</span>
        <span v-if="page" class="hdr-page">第 {{ articlePage }}/{{ page.totalPages }} 页</span>
      </div>
      <button class="hdr-btn" @click="showChapters = !showChapters">目录</button>
    </div>

    <div v-if="showChapters" class="chapter-drawer">
      <button
        v-for="(c, idx) in chapters"
        :key="c.id"
        class="chapter-item"
        :class="{ active: idx === chapterIndex }"
        @click="goChapter(idx)"
      >
        #{{ c.order }} {{ c.title || c.id }}
      </button>
    </div>

    <LoadingSpinner v-if="loading" message="打开小说..." />
    <EmptyState v-else-if="error" title="阅读失败" :message="error">
      <button class="retry-btn" @click="loadPage()">重试</button>
      <button class="retry-btn ghost" @click="router.push('/novels')">返回书架</button>
    </EmptyState>

    <div v-else class="reader-body" :class="{ dim: pageLoading }">
      <div class="reader-text">{{ page?.text }}</div>
    </div>

    <div class="reader-footer">
      <button class="foot-btn" :disabled="!hasPrevPage && !hasPrevChapter" @click="goPage(-1)">
        上一页
      </button>
      <span class="foot-info">
        {{ currentChapter ? `${chapterIndex + 1}/${chapters.length} 章` : '' }}
      </span>
      <button
        class="foot-btn primary"
        :disabled="!hasNextPage && !hasNextChapter"
        @click="goPage(1)"
      >
        下一页
      </button>
    </div>
  </div>
</template>

<style scoped>
.reader-page {
  max-width: 720px;
  margin: 0 auto;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--color-card-bg);
}

.reader-header {
  position: sticky;
  top: 0;
  z-index: 3;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-sm);
  padding: 10px var(--spacing-md);
  background: var(--color-card-bg);
  border-bottom: 1px solid var(--color-hairline);
}

.hdr-btn {
  border: none;
  background: transparent;
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
  padding: 6px 8px;
}

.hdr-center {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
}

.hdr-title {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.hdr-page {
  font-size: 11px;
  color: var(--color-text-secondary);
}

.chapter-drawer {
  position: sticky;
  top: 52px;
  z-index: 2;
  max-height: 40vh;
  overflow-y: auto;
  background: var(--color-canvas-parchment, var(--color-card-bg));
  border-bottom: 1px solid var(--color-hairline);
  display: flex;
  flex-direction: column;
}

.chapter-item {
  text-align: left;
  padding: 10px var(--spacing-md);
  border: none;
  border-bottom: 1px solid var(--color-hairline);
  background: transparent;
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.chapter-item.active {
  color: var(--color-primary);
  font-weight: 600;
}

.reader-body {
  flex: 1;
  padding: var(--spacing-lg) var(--spacing-md) var(--spacing-xl);
  transition: opacity 0.15s ease;
}

.reader-body.dim {
  opacity: 0.45;
}

.reader-text {
  white-space: pre-line;
  font-size: 17px;
  line-height: 1.9;
  color: var(--color-text-primary);
}

.reader-footer {
  position: sticky;
  bottom: 0;
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 10px var(--spacing-md);
  background: var(--color-card-bg);
  border-top: 1px solid var(--color-hairline);
}

.foot-btn {
  flex: 1;
  padding: 10px 0;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.foot-btn.primary {
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: #fff;
}

.foot-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.foot-info {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  white-space: nowrap;
}

.retry-btn {
  margin-top: var(--spacing-sm);
  padding: 8px 24px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  cursor: pointer;
}

.retry-btn.ghost {
  background: transparent;
  border: 1px solid var(--color-border, var(--color-hairline));
  color: var(--color-text-primary);
}
</style>
