<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { novelProgress, novelRemove, novelShelf } from '@/services/api'
import type { NovelSeries } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'
import NovelSeriesCard from '@/components/NovelSeriesCard.vue'

const router = useRouter()
const shelf = ref<NovelSeries[]>([])
const progressMap = ref<Record<string, [number, number]>>({})
const loading = ref(true)

// 签名元素「续读卡」:有阅读进度的系列(阅读连续性是小说板块特有的
// 结构信息,漫画/视频没有)置顶为快捷续读入口;完整书架网格在下方
const readingNow = computed(() =>
  shelf.value.filter((s) => progressMap.value[s.id]),
)

function chapterCount(s: NovelSeries): string {
  return s.chapters.length ? `${s.chapters.length} 章` : '加载中'
}

async function load() {
  loading.value = true
  try {
    shelf.value = await novelShelf()
    for (const s of shelf.value) {
      try {
        const p = await novelProgress(s.id)
        if (p) progressMap.value[s.id] = p
      } catch {
        /* 进度读取失败忽略,视为无记录 */
      }
    }
  } catch (e) {
    // novelShelf 走本地库,失败罕见;文案给出重试路径
    console.error('[shelf]', errorMessage(e))
  } finally {
    loading.value = false
  }
}

/** 点卡片进系列详情页(浏览章节/简介,详情页内再续读) */
function open(series: NovelSeries) {
  router.push(`/novels/series/${series.id}`)
}

async function handleRemove(id: string) {
  try {
    await novelRemove(id)
    shelf.value = shelf.value.filter((s) => s.id !== id)
  } catch {
    /* 移除失败保留条目,用户可重试 */
  }
}

onMounted(() => {
  void load()
})
</script>

<template>
  <div class="shelf-page">
    <div class="shelf-header">
      <button class="back-btn" @click="router.push('/novels')">‹ 发现</button>
      <h2 class="page-title">书架</h2>
    </div>

    <LoadingSpinner v-if="loading" message="加载书架..." />
    <EmptyState
      v-else-if="!shelf.length"
      title="书架还是空的"
      message="去「发现」刷一刷,遇到喜欢的点「+ 书架」收藏"
    >
      <button class="discover-btn" @click="router.push('/novels')">去发现</button>
    </EmptyState>

    <template v-else>
      <section v-if="readingNow.length" class="reading-now">
        <span class="section-label">继续阅读</span>
        <button
          v-for="s in readingNow"
          :key="s.id"
          type="button"
          class="resume-card"
          @click="router.push(`/novels/${s.id}`)"
        >
          <div class="resume-cover">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
          </div>
          <div class="resume-info">
            <span class="resume-title">{{ s.title }}</span>
            <span class="resume-meta">{{ s.author || '未知作者' }} · {{ chapterCount(s) }}</span>
            <span class="resume-progress">
              读到第 {{ progressMap[s.id]![0] }} 章{{ progressMap[s.id]![1] > 1 ? ` 第 ${progressMap[s.id]![1]} 页` : '' }}
            </span>
          </div>
          <span class="resume-cta">续读</span>
        </button>
      </section>

      <section class="all-shelf">
        <span class="section-label">全部收藏</span>
        <div class="shelf-grid">
          <div v-for="s in shelf" :key="s.id" class="shelf-cell">
            <NovelSeriesCard
              :series="s"
              :progress="progressMap[s.id] ?? null"
              @open="open(s)"
            />
            <button
              class="shelf-remove"
              title="移出书架"
              aria-label="移出书架"
              @click.stop="handleRemove(s.id)"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" fill="currentColor" />
              </svg>
            </button>
          </div>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.shelf-page {
  max-width: 720px;
  margin: 0 auto;
  padding: var(--spacing-md) var(--spacing-md) var(--spacing-xl);
}

.shelf-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
}

.back-btn {
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  padding: 6px 12px;
  cursor: pointer;
}

.back-btn:hover {
  border-color: var(--color-primary);
}

.page-title {
  font-size: var(--font-size-headline-title, 20px);
  font-weight: var(--font-weight-bold, 700);
  color: var(--color-text-primary);
  margin: 0;
}

.section-label {
  display: block;
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-sm);
}

.reading-now {
  margin-bottom: var(--spacing-lg);
}

.resume-card {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  width: 100%;
  padding: var(--spacing-sm) var(--spacing-md);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s;
}

.resume-card + .resume-card {
  margin-top: var(--spacing-sm);
}

.resume-card:hover {
  border-color: var(--color-primary);
}

.resume-cover {
  flex-shrink: 0;
  width: 44px;
  height: 44px;
  border-radius: var(--radius-sm);
  background: var(--color-divider-soft);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-muted);
}

.resume-cover svg {
  width: 20px;
  height: 20px;
}

.resume-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.resume-title {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.resume-meta {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
}

.resume-progress {
  font-size: var(--font-size-fine-print);
  color: var(--color-primary);
}

.resume-cta {
  flex-shrink: 0;
  padding: 6px 14px;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-caption);
}

.all-shelf {
  margin-bottom: var(--spacing-xl);
}

.shelf-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: var(--spacing-md);
}

.shelf-cell {
  position: relative;
}

.shelf-remove {
  position: absolute;
  right: 6px;
  top: 6px;
  z-index: 2;
  border: none;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  cursor: pointer;
  padding: 6px;
  border-radius: 50%;
}

.shelf-remove:hover {
  color: var(--color-error);
}
</style>
