<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { errorMessage } from '@/utils/errors'
import { videoFollow, videoFollowList, videoFollowWorks, videoUnfollow } from '@/services/api'
import { dedupeByVideoBase } from '@/utils/videoCode'
import type { FollowTargetEntry, VideoCard } from '@/types'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromVideo } from '@/utils/mediaItem'

const route = useRoute()
const router = useRouter()

// 路由参数:来源(missav/fs1)、类型(maker=厂商/actress=女优)、目标 id
const source = computed(() => String(route.params.source || 'missav'))
const kind = computed(() => String(route.params.kind || 'actress'))
const targetId = computed(() => String(route.params.targetId || ''))

const works = ref<VideoCard[]>([])
const worksPage = ref(1)
const worksHasNext = ref(false)
const worksLoading = ref(false)
const worksError = ref('')
const workItems = computed(() => works.value.map((v) => fromVideo(v)))
/** 请求序号:目标切换后丢弃旧目标的迟到响应 */
let worksSeq = 0

async function loadWorks(reset = true) {
  if (!targetId.value || (!reset && worksLoading.value)) return
  const seq = ++worksSeq
  const page = reset ? 1 : worksPage.value
  worksLoading.value = true
  worksError.value = ''
  try {
    const res = await videoFollowWorks(source.value, kind.value, targetId.value, page)
    if (seq !== worksSeq) return
    works.value = dedupeByVideoBase(reset ? res.items : [...works.value, ...res.items])
    worksPage.value = page + 1
    worksHasNext.value = res.pagination.hasNext
  } catch (e) {
    if (seq === worksSeq) worksError.value = errorMessage(e) || '作品加载失败'
  } finally {
    if (seq === worksSeq) worksLoading.value = false
  }
}

// —— 关注状态(与关注页双向同步:读写同一张 follow_target 表) ——
const followedEntry = ref<FollowTargetEntry | null>(null)
/** 已存的显示名(取关后再关注沿用,避免被 targetId 覆盖) */
const knownName = ref('')
const followStateReady = ref(false)
const followBusy = ref(false)
const followError = ref('')
let followSeq = 0

/** 读取当前目标是否已关注(未就绪前禁用按钮,避免重复关注覆盖已存的显示名) */
async function loadFollowState() {
  const seq = ++followSeq
  followStateReady.value = false
  try {
    const list = await videoFollowList(source.value, kind.value)
    if (seq !== followSeq) return
    followedEntry.value = list.find((f) => f.targetId === targetId.value) ?? null
    knownName.value = followedEntry.value?.displayName ?? ''
  } catch (e) {
    if (seq === followSeq) console.error('[target-works]', errorMessage(e))
  } finally {
    if (seq === followSeq) followStateReady.value = true
  }
}

// —— 取关二次确认:首次点击「待确认」,3 秒内再点才执行(与关注页/历史页一致) ——
const ARM_TIMEOUT_MS = 3000
const unfollowArmed = ref(false)
let armTimer: ReturnType<typeof setTimeout> | undefined
function disarm() {
  clearTimeout(armTimer)
  unfollowArmed.value = false
}
onUnmounted(disarm)

/** 关注/取关:乐观切换,失败回滚并提示 */
async function toggleFollow() {
  if (followBusy.value || !followStateReady.value) return
  const prev = followedEntry.value
  if (prev && !unfollowArmed.value) {
    unfollowArmed.value = true
    armTimer = setTimeout(disarm, ARM_TIMEOUT_MS)
    return
  }
  disarm()
  followBusy.value = true
  followError.value = ''
  const s = source.value
  const k = kind.value
  const id = targetId.value
  const name = knownName.value || id
  followedEntry.value = prev
    ? null
    : { source: s, kind: k, targetId: id, displayName: name, followedAt: Date.now() }
  try {
    if (prev) await videoUnfollow(s, k, id)
    else await videoFollow(s, k, id, name)
  } catch (e) {
    if (id === targetId.value) {
      followedEntry.value = prev
      followError.value = `${prev ? '取消关注' : '关注'}失败:${errorMessage(e)}`
    }
  } finally {
    followBusy.value = false
  }
}

const followLabel = computed(() => {
  if (!followedEntry.value) return '+ 关注'
  return unfollowArmed.value ? '取消关注' : '已关注'
})

const kindLabel = computed(() => (kind.value === 'maker' ? '厂商' : '女优'))
const displayName = computed(() => knownName.value || targetId.value)
const pageTitle = computed(() => `${displayName.value || kindLabel.value}的作品`)

// 同一路由组件复用时(目标 A → 目标 B)参数变化也要整页重置
watch(
  () => [source.value, kind.value, targetId.value],
  () => {
    disarm()
    followError.value = ''
    followedEntry.value = null
    knownName.value = ''
    works.value = []
    worksHasNext.value = false
    void loadWorks(true)
    void loadFollowState()
  },
  { immediate: true },
)
</script>

<template>
  <div class="target-works-page">
    <div class="container">
      <div class="page-heading">
        <button type="button" class="heading-btn is-accent" aria-label="返回" @click="router.back()">
          ‹ 返回
        </button>
        <h2 class="page-title">{{ pageTitle }}</h2>
        <button
          type="button"
          class="heading-btn"
          :class="{ 'is-followed': followedEntry && !unfollowArmed, 'is-danger': unfollowArmed }"
          :disabled="followBusy || !followStateReady"
          :aria-pressed="!!followedEntry"
          @click="toggleFollow"
        >
          {{ followLabel }}
        </button>
      </div>

      <p v-if="followError" class="status-line is-error" role="alert">{{ followError }}</p>

      <MediaCollection
        :items="workItems"
        density="video"
        root-margin="400px"
        :loading="worksLoading && !works.length"
        :loading-more="worksLoading && works.length > 0"
        :has-more="worksHasNext"
        :error="worksError"
        end-text="已加载全部作品"
        empty-title="暂无作品"
        empty-message="该目标下暂无收录作品"
        @load-more="loadWorks(false)"
        @retry="loadWorks(!works.length)"
      />
    </div>
  </div>
</template>

<style scoped>
.target-works-page {
  padding-bottom: var(--spacing-xl);
}

/* —— 页头(关注/历史/目标作品三页同款) —— */
.page-heading {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding-top: var(--spacing-lg);
  margin-bottom: var(--spacing-lg);
}

.page-title {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-heading);
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.heading-btn {
  position: relative;
  flex: none;
  min-height: 36px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  white-space: nowrap;
  transition:
    transform var(--duration-fast) var(--ease-standard),
    opacity var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

/* 视觉 36px,命中区扩到 44px */
.heading-btn::after {
  content: '';
  position: absolute;
  inset: -4px 0;
}

.heading-btn.is-accent {
  color: var(--color-accent);
}

.heading-btn.is-followed {
  color: var(--color-text-muted);
}

.heading-btn.is-danger {
  border-color: var(--color-error);
  color: var(--color-error);
}

.heading-btn:disabled {
  opacity: 0.55;
}

.status-line {
  margin: calc(var(--spacing-sm) * -1) 0 var(--spacing-md);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.status-line.is-error {
  color: var(--color-error);
}
</style>
