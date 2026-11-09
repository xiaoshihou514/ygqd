<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { novelAdd, novelFollowedAuthors, novelFollowAuthor, novelProgress, novelRemove, novelSeriesInfo, novelShelf } from '@/services/api'
import type { NovelSeries } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const route = useRoute()
const router = useRouter()

const series = ref<NovelSeries | null>(null)
const progress = ref<[number, number] | null>(null)
// 收藏 = 关注作者(复用关注机制),数据写接口已就绪;移除保留(novelRemove 真实数据操作)
const authorFollowed = computed(() =>
  !!series.value?.author && novelFollowedIds.value.has(series.value.author),
)
const inShelf = ref(false)
const addingShelf = ref(false)
// 收藏 = 关注作者(小说独立关注链 novel_follow_author;批次 2 关注 tab 数据源)
const novelFollowedIds = ref<Set<string>>(new Set())

async function loadNovelFollows() {
  try {
    novelFollowedIds.value = new Set((await novelFollowedAuthors()).map((f) => f.authorId))
  } catch { /* 关注列表拉取失败忽略 */ }
}
const loading = ref(true)
const error = ref('')
const introExpanded = ref(false)

const id = computed(() => String(route.params.id || ''))
const chapters = computed(() => series.value?.chapters ?? [])
// 登录墙未登录时部分元数据可能拿不到(后端 serde default 空值),展示容错
const authorText = computed(() => series.value?.author || '未知作者')
const introText = computed(() => series.value?.introduction?.trim() || '')
// 简介 meta description 可能含站点长文案,列表态截断、点按展开
const introShort = computed(() =>
  introText.value.length > 120 ? `${introText.value.slice(0, 120)}…` : introText.value,
)
const hasProgress = computed(() => !!progress.value)

async function load() {
  loading.value = true
  error.value = ''
  try {
    // 数据源两级:书架内系列走本地(快,含进度);未入库走远程抓取预览
    // (novel_series_info 不入库,首次撞登录墙经 errorMessage 显示引导)
    const shelf = await novelShelf()
    const inShelfSeries = shelf.find((s) => s.id === id.value)
    if (inShelfSeries) {
      series.value = inShelfSeries
      inShelf.value = true
      try {
        const p = await novelProgress(id.value)
        if (p) progress.value = p
      } catch {
        /* 进度读取失败忽略,视为无记录 */
      }
      return
    }
    series.value = await novelSeriesInfo(id.value)
    inShelf.value = false
  } catch (e) {
    error.value = errorMessage(e) || '加载失败'
  } finally {
    loading.value = false
  }
}

/** 开始阅读/续读:仅书架内系列(阅读器数据源是书架);阅读器按已存
 *  进度自动续(无记录则从第一章开始) */
function startReading() {
  router.push(`/novels/${id.value}`)
}

/** 关注作者:未入库浏览态的收藏动作(批次 2 关注 tab 小说分组数据源) */
async function followNovelAuthor() {
  const a = series.value?.author
  if (!a || authorFollowed.value) return
  try {
    await novelFollowAuthor(a, a)
  } catch (e) {
    error.value = errorMessage(e) || '关注失败,请重试'
  }
}

/** 未入库系列的决策动作:加书架(重新抓取目录并入库),成功后转书架态 */
async function addShelf() {
  if (addingShelf.value) return
  addingShelf.value = true
  error.value = ''
  try {
    await novelAdd(id.value)
    await load()
  } catch (e) {
    error.value = errorMessage(e) || '添加失败'
  } finally {
    addingShelf.value = false
  }
}

/** 目录点选直达指定章节(阅读器支持 query.chapter 覆盖续读进度) */
function readChapter(order: number) {
  router.push({ path: `/novels/${id.value}`, query: { chapter: String(order) } })
}

async function handleRemove() {
  try {
    await novelRemove(id.value)
    router.replace('/novels')
  } catch {
    /* 移除失败保留在详情页,用户可重试 */
  }
}

onMounted(() => {
  void loadNovelFollows()
  void load()
})
</script>

<template>
  <div class="series-detail-page">
    <div class="detail-header">
      <button class="back-btn" @click="router.push('/novels')">‹ 小说</button>
    </div>

    <LoadingSpinner v-if="loading" message="加载系列..." />
    <EmptyState v-else-if="error" title="加载失败" :message="error">
      <button class="retry-btn" @click="load()">重试</button>
      <button class="retry-btn ghost" @click="router.push('/novels')">返回小说</button>
    </EmptyState>

    <template v-else-if="series">
      <div class="detail-hero">
        <div class="hero-cover">
          <!-- hlib 不提供系列封面(cover 恒空串):占位图标,与书架卡一致 -->
          <div class="cover-fallback">
            <svg class="fallback-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
            <span class="fallback-label">暂无封面</span>
          </div>
        </div>
        <div class="hero-info">
          <h2 class="hero-title">{{ series.title }}</h2>
          <span class="hero-author">{{ authorText }}</span>
          <span class="hero-count">{{ chapters.length }} 章</span>
          <button
            v-if="inShelf"
            class="read-btn primary"
            @click="startReading"
          >
            {{ hasProgress ? `续读 第 ${progress![0]} 章` : '开始阅读' }}
          </button>
          <!-- 未入库系列:直接阅读(R2 数据源两级支持,阅读进度自动保留);
               收藏(关注作者)批次 2 上线后在此恢复按钮 -->
          <button class="read-btn primary" @click="startReading">开始阅读</button>
          <button
            v-if="series.author && !authorFollowed"
            class="read-btn ghost"
            @click="followNovelAuthor"
          >
            + 关注作者
          </button>
          <button
            v-else-if="series.author && authorFollowed"
            class="read-btn ghost"
            disabled
          >
            已关注作者
          </button>
          <button v-if="inShelf" class="read-btn ghost" @click="handleRemove">移除</button>
        </div>
      </div>

      <div v-if="introText" class="intro-block">
        <span class="intro-text">{{ introExpanded ? introText : introShort }}</span>
        <button
          v-if="introText.length > 120"
          type="button"
          class="intro-toggle"
          @click="introExpanded = !introExpanded"
        >
          {{ introExpanded ? '收起' : '展开' }}
        </button>
      </div>

      <div class="chapter-block">
        <span class="chapter-heading">章节列表</span>
        <div class="chapter-list">
          <button
            v-for="c in chapters"
            :key="c.id"
            type="button"
            class="chapter-row"
            :class="{ current: progress && progress[0] === c.order }"
            @click="readChapter(c.order)"
          >
            <span class="chapter-label">#{{ c.order }} {{ c.title || c.id }}</span>
            <span v-if="progress && progress[0] === c.order" class="chapter-here">当前读到</span>
          </button>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.series-detail-page {
  max-width: 720px;
  margin: 0 auto;
  padding: var(--spacing-md) var(--spacing-md) var(--spacing-xl);
}

.detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--spacing-md);
}

.back-btn,
.back-btn:hover,

.detail-hero {
  display: flex;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-lg);
}

.hero-cover {
  flex-shrink: 0;
  width: 120px;
  aspect-ratio: 3 / 4;
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--color-divider-soft);
}

.cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  color: var(--color-text-muted);
}

.fallback-icon {
  width: 32px;
  height: 32px;
  opacity: 0.5;
}

.fallback-label {
  font-size: 11px;
  color: var(--color-text-secondary);
}

.hero-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--spacing-xs);
}

.hero-title {
  font-size: var(--font-size-headline-title, 20px);
  font-weight: var(--font-weight-bold, 700);
  color: var(--color-text-primary);
  margin: 0;
  overflow-wrap: anywhere;
}

.hero-author {
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
}

.hero-count {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.read-btn {
  width: 100%;
  padding: 9px 0;
  border-radius: var(--radius-pill, 999px);
  font-size: var(--font-size-caption);
  cursor: pointer;
  margin-top: var(--spacing-xs);
}

.read-btn.primary {
  border: none;
  background: var(--color-primary);
  color: #fff;
}

.read-btn.primary:hover {
  background: var(--color-primary-focus);
}

.read-btn.ghost {
  border: 1px solid var(--color-hairline);
  background: transparent;
  color: var(--color-text-secondary);
}

.intro-block {
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  padding: var(--spacing-md);
  margin-bottom: var(--spacing-lg);
}

.intro-text {
  font-size: var(--font-size-caption);
  line-height: 1.7;
  color: var(--color-text-secondary);
  overflow-wrap: anywhere;
}

.intro-toggle {
  border: none;
  background: transparent;
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
  padding: 0 4px;
}

.chapter-block {
  margin-bottom: var(--spacing-xl);
}

.chapter-heading {
  display: block;
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-sm);
}

.chapter-list {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  overflow: hidden;
  max-height: 60vh;
  overflow-y: auto;
}

.chapter-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-sm);
  padding: 10px var(--spacing-md);
  border: none;
  border-bottom: 1px solid var(--color-hairline);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  text-align: left;
  cursor: pointer;
}

.chapter-row:hover {
  background: var(--color-primary-focus);
}

.chapter-row:last-child {
  border-bottom: none;
}

.chapter-row.current {
  background: var(--color-primary-focus);
}

.chapter-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chapter-here {
  flex-shrink: 0;
  font-size: var(--font-size-fine-print);
  color: var(--color-primary);
}
</style>
