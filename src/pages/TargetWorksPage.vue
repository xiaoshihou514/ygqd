<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { videoFollow, videoFollowWorks, videoUnfollow } from '@/services/api'
import type { FollowTargetEntry, VideoCard } from '@/types'
import VideoCardGrid from '@/components/VideoCardGrid.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const route = useRoute()
const router = useRouter()

// 路由参数:来源(missav/fs1)、类型(maker=厂商/actress=女优)、目标 id/名称
const source = computed(() => String(route.params.source || 'missav'))
const kind = computed(() => String(route.params.kind || 'actress'))
const targetId = computed(() => String(route.params.targetId || ''))

const works = ref<VideoCard[]>([])
const worksPage = ref(1)
const worksHasNext = ref(false)
const worksLoading = ref(false)
const worksError = ref('')

// 关注状态(详情页/关注 tab 双入口共用的关注与浏览通道)
const followedEntry = ref<FollowTargetEntry | null>(null)
const followBusy = ref(false)

/** base_code 优先级去重:中字 > 有码 > 无码 > 英字(需求 12 同款逻辑) */
function baseCode(code: string): string {
  return code
    .replace('-uncensored-leak-chinese-subtitle', '')
    .replace('-uncensored-leak', '')
    .replace('-chinese-subtitle', '')
}

function dedupeByBase(items: VideoCard[]): VideoCard[] {
  const priority = (c: string): number => {
    if (c.includes('-chinese-subtitle')) return 3
    if (!c.includes('-uncensored-leak')) return 2
    return 1
  }
  const best = new Map<string, VideoCard>()
  for (const item of items) {
    const base = baseCode(item.code)
    const cur = best.get(base)
    if (!cur || priority(item.code) > priority(cur.code)) best.set(base, item)
  }
  return [...best.values()]
}

async function loadWorks(reset = true) {
  if (worksLoading.value || !targetId.value) return
  if (reset) worksPage.value = 1
  worksLoading.value = true
  worksError.value = ''
  try {
    const res = await videoFollowWorks(source.value, kind.value, targetId.value, worksPage.value)
    if (reset) {
      works.value = dedupeByBase(res.items)
    } else {
      works.value = dedupeByBase([...works.value, ...res.items])
    }
    worksPage.value += 1
    worksHasNext.value = res.pagination.hasNext
  } catch (e) {
    worksError.value = errorMessage(e) || '作品加载失败'
  } finally {
    worksLoading.value = false
  }
}

/** 哨兵滚动加载(视频源无反爬闸,无限滚动无节流) */
function setupSentinel() {
  const el = document.querySelector('.target-scroll-sentinel')
  if (!el) return
  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0]?.isIntersecting) void loadWorks(false)
    },
    { rootMargin: '400px' },
  )
  observer.observe(el)
}

let observer: IntersectionObserver | null = null

/** 关注/取关(与关注 tab 双向同步:关注后关注列表即时更新) */
async function toggleFollow() {
  if (followBusy.value) return
  followBusy.value = true
  try {
    if (followedEntry.value) {
      await videoUnfollow(source.value, kind.value, targetId.value)
      followedEntry.value = null
    } else {
      await videoFollow(source.value, kind.value, targetId.value, targetId.value)
      followedEntry.value = {
        source: source.value,
        kind: kind.value,
        targetId: targetId.value,
        displayName: targetId.value,
        followedAt: Date.now(),
      }
    }
  } catch (e) {
    worksError.value = errorMessage(e) || '操作失败,请重试'
  } finally {
    followBusy.value = false
  }
}

const kindLabel = computed(() => (kind.value === 'maker' ? '厂商' : '女优'))
const pageTitle = computed(() => `${targetId.value || kindLabel.value}的作品`)

onMounted(async () => {
  await loadWorks(true)
  setupSentinel()
})

onUnmounted(() => {
  observer?.disconnect()
})
</script>

<template>
  <div class="target-works-page">
    <div class="page-header">
      <button class="back-btn" @click="router.back()">‹ 返回</button>
      <h2 class="page-title">{{ pageTitle }}</h2>
      <button
        type="button"
        class="follow-toggle"
        :disabled="followBusy"
        @click="toggleFollow"
      >
        {{ followedEntry ? '已关注' : '+ 关注' }}
      </button>
    </div>

    <LoadingSpinner v-if="worksLoading && !works.length" message="加载作品..." />
    <EmptyState v-else-if="worksError && !works.length" title="加载失败" :message="worksError">
      <button class="retry-btn" @click="loadWorks(true)">重试</button>
    </EmptyState>
    <EmptyState v-else-if="!works.length" title="暂无作品" message="该目标下暂无收录作品" />

    <template v-else>
      <VideoCardGrid :items="works" />
      <div v-if="worksHasNext" class="target-scroll-sentinel" />
      <div v-if="worksLoading" class="loading-more">
        <LoadingSpinner message="正在加载更多..." />
      </div>
      <div v-if="!worksHasNext && works.length > 0" class="no-more">
        <span>已加载全部作品</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.target-works-page {
  max-width: none;
  padding-bottom: var(--spacing-xl);
}

.page-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: var(--spacing-md) var(--spacing-lg);
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
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.follow-toggle {
  flex-shrink: 0;
  padding: 6px 16px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.follow-toggle:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.follow-toggle:disabled {
  opacity: 0.5;
  cursor: default;
}

.loading-more,
.no-more {
  display: flex;
  justify-content: center;
  padding: var(--spacing-md) 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
}
</style>
