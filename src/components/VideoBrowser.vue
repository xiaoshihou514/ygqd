<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { fetchVideoList, fetchVideoWatchedIds } from '@/services/api'
import { useInfiniteScroll } from '@/composables/useInfiniteScroll'
import { videoBaseCode } from '@/utils/videoCode'
import { formatCount } from '@/utils/formatCount'
import { parseLikes } from '@/utils/likes'
import type { VideoCard } from '@/types'
import ProxyImage from '@/components/ProxyImage.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const router = useRouter()
const items = ref<VideoCard[]>([])
const loading = ref(true)
const loadingMore = ref(false)
const error = ref('')
const page = ref(1)
const hasNext = ref(true)
const watched = ref<Set<string>>(new Set())
let observer: IntersectionObserver | null = null
/** 常驻哨兵节点(v-show 显隐),不随条件分支销毁重建,observer 一次绑定终身有效 */
const sentinelEl = ref<HTMLElement | null>(null)

async function load(reset: boolean) {
  if (reset) {
    loading.value = true
    error.value = ''
    page.value = 1
  } else {
    // 追加加载拦截:并发锁 / 无下一页 / 首载中 / 无数据(首载失败或空结果,防止悄悄拉下一页)
    const blocked = loadingMore.value || !hasNext.value || loading.value || !items.value.length
    if (blocked) {
      console.info('[video-browser] 追加加载拦截', {
        loadingMore: loadingMore.value,
        hasNext: hasNext.value,
        loading: loading.value,
        items: items.value.length,
      })
      return
    }
    loadingMore.value = true
  }
  try {
    const result = await fetchVideoList(page.value)
    if (reset) {
      items.value = result.items
    } else {
      const seen = new Set(items.value.map((v) => v.code))
      items.value.push(...result.items.filter((v) => !seen.has(v.code)))
    }
    hasNext.value = result.pagination.hasNext
    // 批量查询已看,卡片标灰角标
    try {
      const ids = await fetchVideoWatchedIds(items.value.map((v) => v.code))
      watched.value = new Set(ids)
    } catch { /* 查询失败不影响列表 */ }
    page.value += 1
  } catch (e) {
    error.value = errorMessage(e) || '加载失败'
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}









/** 徽标展示用基础番号:剥无码流出与中字后缀(先长后短,与后端 title
 *  剥离同序)。跳转 / 去重 / 已看记录仍用完整 code,不受展示层影响。 */
/** 哨兵常驻 DOM(v-show 显隐):display:none 的元素与视口不相交,
 *  隐藏期间不触发回调,恢复显示即自动恢复工作——节点不随条件分支
 *  销毁重建,observer 一次绑定终身有效(前两轮「搜索后翻页失效」的
 *  根因即哨兵节点重建导致观察失联,本结构从根上消除该模式)。 */
const { setup: setupInfiniteScroll } = useInfiniteScroll({
  target: () => sentinelEl.value,
  onHit: () => void load(false),
  rootMargin: '400px',
})

onMounted(() => {
  setupInfiniteScroll()
  void load(true)
})
</script>

<template>
  <div class="video-browser">
      <LoadingSpinner v-if="loading" message="加载中..." />
      <EmptyState
        v-else-if="error"
        title="加载失败"
        :message="error"
      />
      <EmptyState
        v-else-if="!items.length"
        title="暂无内容"
        message="暂无内容"
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
            <span v-if="card.views" class="video-views">▶ {{ formatCount(parseLikes(card.views)) }}</span>
            <span v-if="watched.has(card.code)" class="video-watched">已看</span>
          </div>
          <div class="video-card-body">
            <span class="video-code-row">
              <span class="video-code">{{ videoBaseCode(card.code) }}</span>
              <span v-if="card.code.includes('-uncensored-leak')" class="video-leak-badge">无码</span>
              <span v-if="card.chinese" class="video-cs-badge">中字</span>
            </span>
            <span class="video-card-title">{{ card.title || videoBaseCode(card.code) }}</span>
          </div>
        </button>
      </div>

      <div v-if="loadingMore" class="loading-more">
        <LoadingSpinner message="正在加载更多..." />
      </div>
      <!-- 常驻哨兵:v-show 显隐(display:none 不与视口相交,隐藏期间不触发),
           节点不销毁重建,observer 绑定终身有效 -->
      <div v-show="!loading && !error && items.length > 0" ref="sentinelEl" class="scroll-sentinel" />
      <div v-if="!loading && !error && !hasNext && items.length" class="no-more">
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

.video-watched {
  position: absolute;
  left: 6px;
  top: 6px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.65);
  color: #9be9a8;
  font-size: 11px;
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

.video-leak-badge {
  padding: 0 5px;
  border-radius: 4px;
  background: #6741d9;
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
