<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { novelAdd, novelProgress, novelRead, novelSeriesInfo, novelSetProgress, novelShelf } from '@/services/api'
import type { NovelPage, NovelSeries } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const route = useRoute()
const router = useRouter()

const series = ref<NovelSeries | null>(null)
/** 是否已在书架:未入库系列(novel_series_info 预览)阅读中可加书架 */
const inShelf = ref(false)
const addingShelf = ref(false)
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

// 阅读设置:纯前端 localStorage(对齐搜索历史先例,零后端依赖)
type ReaderFontSize = 'sm' | 'md' | 'lg'
type ReaderLineHeight = 'compact' | 'normal' | 'relaxed'
type ReaderTheme = 'paper' | 'dark' | 'green'
const SETTINGS_KEY = 'niacg-reader-settings'
const settings = ref({
  fontSize: 'md' as ReaderFontSize,
  lineHeight: 'normal' as ReaderLineHeight,
  theme: 'paper' as ReaderTheme,
})
const showSettings = ref(false)
// 面板与目录互斥开合(叠开视觉怪,backend-dev 审查裁决)
function toggleSettings() {
  showSettings.value = !showSettings.value
  if (showSettings.value) showChapters.value = false
}

function toggleChapters() {
  showChapters.value = !showChapters.value
  if (showChapters.value) showSettings.value = false
}
const FONT_SIZE_PX: Record<ReaderFontSize, string> = { sm: '15px', md: '17px', lg: '19px' }
const LINE_HEIGHTS: Record<ReaderLineHeight, string> = {
  compact: '1.6',
  normal: '1.8',
  relaxed: '2.1',
}
const FONT_SIZE_OPTIONS: Array<{ key: ReaderFontSize; label: string }> = [
  { key: 'sm', label: '小' },
  { key: 'md', label: '标准' },
  { key: 'lg', label: '大' },
]
const LINE_HEIGHT_OPTIONS: Array<{ key: ReaderLineHeight; label: string }> = [
  { key: 'compact', label: '紧凑' },
  { key: 'normal', label: '标准' },
  { key: 'relaxed', label: '宽松' },
]
const THEME_OPTIONS: Array<{ key: ReaderTheme; label: string }> = [
  { key: 'paper', label: '纸白' },
  { key: 'dark', label: '暗黑' },
  { key: 'green', label: '护眼绿' },
]
const textStyle = computed(() => ({
  fontSize: FONT_SIZE_PX[settings.value.fontSize],
  lineHeight: LINE_HEIGHTS[settings.value.lineHeight],
}))

function loadSettings() {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (!raw) return
    const parsed = JSON.parse(raw) as Partial<{
      fontSize: ReaderFontSize
      lineHeight: ReaderLineHeight
      theme: ReaderTheme
    }>
    if (parsed.fontSize && parsed.fontSize in FONT_SIZE_PX) settings.value.fontSize = parsed.fontSize
    if (parsed.lineHeight && parsed.lineHeight in LINE_HEIGHTS) {
      settings.value.lineHeight = parsed.lineHeight
    }
    if (parsed.theme && ['paper', 'dark', 'green'].includes(parsed.theme)) {
      settings.value.theme = parsed.theme as ReaderTheme
    }
  } catch {
    /* 损坏则用默认值 */
  }
}

function setSetting<K extends 'fontSize' | 'lineHeight' | 'theme'>(
  key: K,
  value: (typeof settings.value)[K],
) {
  settings.value[key] = value
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings.value))
}

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
    error.value = errorMessage(e) || '加载失败'
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

/** 阅读中收藏:未入库系列加书架,成功后转书架态(按钮收起) */
async function addShelf() {
  if (addingShelf.value || inShelf.value) return
  addingShelf.value = true
  try {
    await novelAdd(id)
    inShelf.value = true
  } catch (e) {
    error.value = errorMessage(e) || '加书架失败,请重试'
  } finally {
    addingShelf.value = false
  }
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
  loadSettings()
  const id = String(route.params.id || '')
  try {
    // 数据源两级:书架内系列走本地;未入库(详情页/搜索直进)走
    // novel_series_info 预览——支持「阅读中加书架」的收藏闭环
    const shelf = await novelShelf()
    const inShelfSeries = shelf.find((s) => s.id === id)
    if (inShelfSeries) {
      series.value = inShelfSeries
      inShelf.value = true
    } else {
      series.value = await novelSeriesInfo(id)
      inShelf.value = false
    }
    if (!series.value) {
      error.value = '书架中找不到该系列,请重新添加'
      loading.value = false
      return
    }
    // 详情页「章节列表」直达:query.chapter(源站章节序号)优先于续读进度
    const chapterQuery = route.query.chapter
    const targetOrder = chapterQuery ? Number(String(chapterQuery)) : null
    if (targetOrder && Number.isFinite(targetOrder)) {
      const idx = chapters.value.findIndex((c) => c.order === targetOrder)
      if (idx >= 0) {
        chapterIndex.value = idx
        articlePage.value = 1
        await loadPage()
        return
      }
    }
    jumpToProgress()
  } catch (e) {
    error.value = errorMessage(e) || '加载失败'
    loading.value = false
  }
})
</script>

<template>
  <div class="reader-page" :class="`theme-${settings.theme}`">
    <div class="reader-header">
      <button class="hdr-btn" @click="router.push('/novels')">‹ 书架</button>
      <div class="hdr-center">
        <span class="hdr-title">{{ currentChapter ? `#${currentChapter.order} ${currentChapter.title || currentChapter.id}` : (series?.title ?? '') }}</span>
        <span v-if="page" class="hdr-page">第 {{ articlePage }}/{{ page.totalPages }} 页</span>
      </div>
      <div class="hdr-actions">
        <button
          v-if="!inShelf"
          class="hdr-btn shelf-add"
          :disabled="addingShelf"
          @click="addShelf"
        >
          {{ addingShelf ? '收藏中…' : '+ 书架' }}
        </button>
        <button class="hdr-btn" @click="toggleSettings">Aa</button>
        <button class="hdr-btn" @click="toggleChapters">目录</button>
      </div>
    </div>

    <div v-if="showSettings" class="settings-panel">
      <div class="set-group">
        <span class="set-label">字号</span>
        <div class="set-chips">
          <button
            v-for="opt in FONT_SIZE_OPTIONS"
            :key="opt.key"
            type="button"
            class="set-chip"
            :class="{ active: settings.fontSize === opt.key }"
            @click="setSetting('fontSize', opt.key)"
          >
            {{ opt.label }}
          </button>
        </div>
      </div>
      <div class="set-group">
        <span class="set-label">行距</span>
        <div class="set-chips">
          <button
            v-for="opt in LINE_HEIGHT_OPTIONS"
            :key="opt.key"
            type="button"
            class="set-chip"
            :class="{ active: settings.lineHeight === opt.key }"
            @click="setSetting('lineHeight', opt.key)"
          >
            {{ opt.label }}
          </button>
        </div>
      </div>
      <div class="set-group">
        <span class="set-label">背景</span>
        <div class="set-chips">
          <button
            v-for="opt in THEME_OPTIONS"
            :key="opt.key"
            type="button"
            class="set-chip"
            :class="{ active: settings.theme === opt.key }"
            @click="setSetting('theme', opt.key)"
          >
            {{ opt.label }}
          </button>
        </div>
      </div>
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
      <div class="reader-text" :style="textStyle">{{ page?.text }}</div>
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
  background: var(--rd-bg, var(--color-card-bg));
}

/* 阅读主题局部 token:hex 集中定义在此(阅读场景专属配色,与主界面
   token 体系隔离),其余规则一律 var 引用 */
.reader-page.theme-paper {
  --rd-bg: #f7f3ea;
  --rd-fg: #2c2a26;
  --rd-muted: rgba(44, 42, 38, 0.55);
  --rd-line: rgba(44, 42, 38, 0.1);
  --rd-accent: #8a6d2f;
}

.reader-page.theme-dark {
  --rd-bg: #141414;
  --rd-fg: #b8b2a8;
  --rd-muted: rgba(184, 178, 168, 0.5);
  --rd-line: rgba(255, 255, 255, 0.09);
  --rd-accent: #c9a961;
}

.reader-page.theme-green {
  --rd-bg: #cde8cd;
  --rd-fg: #23372a;
  --rd-muted: rgba(35, 55, 42, 0.55);
  --rd-line: rgba(35, 55, 42, 0.14);
  --rd-accent: #3d6b47;
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
  background: var(--rd-bg, var(--color-card-bg));
  border-bottom: 1px solid var(--rd-line, var(--color-hairline));
}

.hdr-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}

.settings-panel {
  position: sticky;
  top: 47px;
  z-index: 3;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: var(--spacing-md);
  background: var(--rd-bg, var(--color-card-bg));
  border-bottom: 1px solid var(--rd-line, var(--color-hairline));
}

.set-group {
  display: flex;
  align-items: center;
  gap: 12px;
}

.set-label {
  min-width: 36px;
  font-size: var(--font-size-fine-print);
  color: var(--rd-muted, var(--color-text-secondary));
}

.set-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  flex: 1;
}

.set-chip {
  padding: 4px 14px;
  border: 1px solid var(--rd-line, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--rd-fg, var(--color-text-primary));
  font-size: var(--font-size-fine-print);
  cursor: pointer;
}

.set-chip.active {
  background: var(--rd-accent, var(--color-primary));
  border-color: var(--rd-accent, var(--color-primary));
  color: var(--rd-bg, var(--color-card-bg));
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
  color: var(--rd-fg, var(--color-text-primary));
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.hdr-page {
  font-size: 11px;
  color: var(--rd-muted, var(--color-text-secondary));
}

.chapter-drawer {
  position: sticky;
  top: 52px;
  z-index: 2;
  max-height: 40vh;
  overflow-y: auto;
  background: var(--rd-bg, var(--color-canvas-parchment, var(--color-card-bg)));
  border-bottom: 1px solid var(--rd-line, var(--color-hairline));
  display: flex;
  flex-direction: column;
}

.chapter-item {
  text-align: left;
  padding: 10px var(--spacing-md);
  border: none;
  border-bottom: 1px solid var(--rd-line, var(--color-hairline));
  background: transparent;
  color: var(--rd-fg, var(--color-text-primary));
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.chapter-item.active {
  color: var(--rd-accent, var(--color-primary));
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
  color: var(--rd-fg, var(--color-text-primary));
}

.reader-footer {
  position: sticky;
  bottom: 0;
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 10px var(--spacing-md);
  background: var(--rd-bg, var(--color-card-bg));
  border-top: 1px solid var(--rd-line, var(--color-hairline));
}

.foot-btn {
  flex: 1;
  padding: 10px 0;
  border: 1px solid var(--rd-line, var(--color-border, var(--color-hairline)));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--rd-fg, var(--color-text-primary));
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
  color: var(--rd-muted, var(--color-text-secondary));
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
