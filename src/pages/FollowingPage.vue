<script setup lang="ts">
import { computed, nextTick, onActivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { errorMessage } from '@/utils/errors'
import { formatCount } from '@/utils/formatCount'
import {
  searchComics,
  videoFollow,
  videoFollowList,
  videoFollowTargets,
  videoFollowWorks,
  videoUnfollow,
} from '@/services/api'
import type { ComicItem, FollowedAuthor, FollowTargetEntry, FollowTargetItem, VideoCard } from '@/types'
import { useFollowedAuthors } from '@/composables/useFollowedAuthors'
import { fromComic, fromVideo } from '@/utils/mediaItem'
import { dedupeByVideoBase } from '@/utils/videoCode'
import MediaCollection from '@/components/MediaCollection.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const router = useRouter()
const { followed, load: loadFollowedAuthors, unfollow } = useFollowedAuthors()

// —— 视图切换(分段控件)与滚动记忆 ——
const tabs = [
  { key: 'authors', label: '漫画作者' },
  { key: 'videos', label: '视频' },
] as const
type FollowingTab = (typeof tabs)[number]['key']

const activeTab = ref<FollowingTab>('authors')
const activeTabIndex = computed(() => tabs.findIndex((t) => t.key === activeTab.value))
const tabScroll: Record<FollowingTab, number> = { authors: 0, videos: 0 }

async function switchTab(tab: FollowingTab) {
  if (tab === activeTab.value) return
  tabScroll[activeTab.value] = window.scrollY
  activeTab.value = tab
  await nextTick()
  window.scrollTo(0, tabScroll[tab])
}

/** 操作失败提示(取关/关注回滚后显示,下一次操作前清空) */
const notice = ref('')

// —— 删除类操作二次确认:首次点击「待确认」,3 秒内再点才执行 ——
const ARM_TIMEOUT_MS = 3000
const armedKey = ref<string | null>(null)
let armTimer: ReturnType<typeof setTimeout> | undefined
function confirmTap(key: string, action: () => void) {
  clearTimeout(armTimer)
  if (armedKey.value === key) {
    armedKey.value = null
    action()
    return
  }
  armedKey.value = key
  armTimer = setTimeout(() => (armedKey.value = null), ARM_TIMEOUT_MS)
}
onUnmounted(() => clearTimeout(armTimer))

// ============================================================
// 漫画作者:关注作者的作品聚合(niacg + bika 双源,作者轮转分页)
// ============================================================
type ComicSource = 'niacg' | 'bika'
const COMIC_SOURCES: readonly ComicSource[] = ['niacg', 'bika']

interface SourcePageState {
  page: number
  hasNext: boolean
}

interface AuthorWorksState {
  items: ComicItem[]
  niacg: SourcePageState
  bika: SourcePageState
  loading: boolean
}

const authorsReady = ref(false)
const authorWorks = ref<Record<string, AuthorWorksState>>({})
const syncing = ref(false)
const syncProgress = ref('')
const syncError = ref('')
const bikaHint = ref('')
const loadingMore = ref(false)
let syncCacheBuster = ''
/** 会话级 buster:进页面时命中后端 5 分钟缓存,避开 niacg 搜索限速的串行等待 */
const SESSION_BUSTER = String(Date.now())
/** 后台刷新阈值:与后端缓存 TTL 对齐,未过期且关注名单未变时不重拉 */
const STALE_MS = 5 * 60 * 1000
const CONCURRENCY = 3
let nextAuthorIndex = 0
let loadedAuthorsKey = ''
let loadedAuthorsAt = 0

const authorsKey = () => followed.value.map((f) => f.author).join('\n')
const comicKey = (item: ComicItem) => `${item.source}:${item.id}`

/** 去重追加(原地修改 target) */
function appendUnique(target: ComicItem[], incoming: ComicItem[]) {
  const seen = new Set(target.map(comicKey))
  for (const item of incoming) {
    const key = comicKey(item)
    if (seen.has(key)) continue
    seen.add(key)
    target.push(item)
  }
}

const allWorks = computed<ComicItem[]>(() => {
  const merged: ComicItem[] = []
  for (const fa of followed.value) appendUnique(merged, authorWorks.value[fa.author]?.items ?? [])
  return merged.sort((a, b) => (Number(b.id) || 0) - (Number(a.id) || 0))
})

// 关注页两组统一交互:对象芯片点选看作品(漫画多一个「全部」),× 取消关注
const selectedComicAuthor = ref<string | null>(null)
const allWorkItems = computed(() => {
  const author = selectedComicAuthor.value
  const list = author ? (authorWorks.value[author]?.items ?? []) : allWorks.value
  return list.map((c) => fromComic(c))
})
const hasMoreWorks = computed(() =>
  followed.value.some((fa) => {
    const state = authorWorks.value[fa.author]
    return !!state && (state.niacg.hasNext || state.bika.hasNext)
  }),
)

async function loadFollows() {
  try {
    await loadFollowedAuthors()
  } catch (e) {
    console.error('[following]', errorMessage(e))
  } finally {
    authorsReady.value = true
  }
}

async function fetchAuthorPage(
  follow: FollowedAuthor,
  source: ComicSource,
  page: number,
  state: AuthorWorksState,
): Promise<void> {
  const result = await searchComics(
    source,
    {
      source,
      keyword: follow.author,
      classid: 9,
      show: source === 'bika' ? 'writer' : 'title,text,keyboard,ftitle',
      tempid: '1',
    },
    page,
    syncCacheBuster,
  )
  appendUnique(state.items, result.items)
  state[source] = { page, hasNext: result.pagination.hasNext }
}

/**
 * 拉取全部关注作者的首页作品。
 * - initial:首次进入,逐个作者渐进展示,命中会话缓存
 * - refresh:用户点刷新,强制绕过缓存,保留旧列表直到新结果就绪(不闪骨架)
 * - background:keep-alive 返回时静默刷新,同上但不显示进度文案
 */
async function loadAllAuthors(mode: 'initial' | 'refresh' | 'background' = 'initial') {
  if (syncing.value || followed.value.length === 0) return
  const keepItems = mode !== 'initial' && allWorks.value.length > 0
  const showProgress = mode !== 'background'
  syncing.value = true
  syncCacheBuster = mode === 'refresh' ? String(Date.now()) : SESSION_BUSTER
  loadedAuthorsKey = authorsKey()
  loadedAuthorsAt = Date.now()
  nextAuthorIndex = 0

  // 保留旧列表时写入独立对象、结束后整体替换;否则直接写响应式对象渐进展示
  let target: Record<string, AuthorWorksState>
  if (keepItems) {
    target = {}
  } else {
    authorWorks.value = {}
    target = authorWorks.value
  }

  let niacgFailed = false
  let bikaNeedsLogin = false
  const queue = [...followed.value]
  const total = queue.length
  let done = 0
  const worker = async () => {
    let follow: FollowedAuthor | undefined
    while ((follow = queue.shift())) {
      const author = follow
      target[author.author] = {
        items: [],
        niacg: { page: -1, hasNext: true },
        bika: { page: -1, hasNext: true },
        loading: true,
      }
      const state = target[author.author]!
      const settled = await Promise.allSettled(
        COMIC_SOURCES.map((source) => fetchAuthorPage(author, source, 0, state)),
      )
      state.loading = false
      settled.forEach((result, index) => {
        if (result.status === 'fulfilled') return
        const source = COMIC_SOURCES[index]!
        state[source].hasNext = false
        if (source === 'niacg') niacgFailed = true
        else if (String(result.reason).includes('登录')) bikaNeedsLogin = true
      })
      done++
      if (showProgress) syncProgress.value = `正在加载 ${total} 位作者的作品(${done}/${total})`
    }
  }
  await Promise.all(Array.from({ length: Math.min(CONCURRENCY, total) }, worker))

  if (keepItems) authorWorks.value = target
  syncError.value = niacgFailed ? '部分作者作品加载失败,可稍后刷新重试' : ''
  bikaHint.value = bikaNeedsLogin ? '部分内容源未连接,可在设置中登录后查看更多作品' : ''
  syncing.value = false
  syncProgress.value = ''
}

/** 作者轮转翻页:每次取下一位仍有下一页的作者,niacg 优先、bika 次之 */
function pickNextAuthorPage() {
  const count = followed.value.length
  for (let offset = 0; offset < count; offset += 1) {
    const index = (nextAuthorIndex + offset) % count
    const follow = followed.value[index]
    const state = follow ? authorWorks.value[follow.author] : undefined
    if (!follow || !state || state.loading) continue
    const source = COMIC_SOURCES.find((s) => state[s].hasNext)
    if (!source) continue
    nextAuthorIndex = (index + 1) % count
    return { follow, source, page: state[source].page + 1, state }
  }
  return null
}

async function loadNextPage() {
  if (loadingMore.value || syncing.value || followed.value.length === 0) return
  const next = pickNextAuthorPage()
  if (!next) return

  loadingMore.value = true
  next.state.loading = true
  try {
    await fetchAuthorPage(next.follow, next.source, next.page, next.state)
  } catch {
    next.state[next.source].hasNext = false
    syncError.value = '加载更多作品失败,请重试'
  } finally {
    next.state.loading = false
    loadingMore.value = false
  }
}

/** 乐观取关:先从列表移除,失败回滚 */
async function unfollowAuthor(author: string) {
  notice.value = ''
  const prev = followed.value
  followed.value = prev.filter((f) => f.author !== author)
  if (selectedComicAuthor.value === author) selectedComicAuthor.value = null
  try {
    await unfollow(author)
    loadedAuthorsKey = authorsKey()
  } catch (e) {
    followed.value = prev
    notice.value = `取消关注失败:${errorMessage(e)}`
  }
}

// ============================================================
// 视频关注:厂商/女优分组 + 关注芯片 + 管理关注 + 作品预览
// ============================================================
type VideoKind = 'actress' | 'maker'
const VIDEO_SOURCE = 'missav'
const VIDEO_KINDS: Array<{ key: VideoKind; label: string }> = [
  { key: 'actress', label: '女优' },
  { key: 'maker', label: '厂商' },
]

const videoKind = ref<VideoKind>('actress')
const videoKindIndex = computed(() => VIDEO_KINDS.findIndex((k) => k.key === videoKind.value))
const videoManage = ref(false)
const videoFollows = ref<FollowTargetEntry[]>([])
const videoFollowsLoading = ref(false)
const videoTargets = ref<FollowTargetItem[]>([])
const videoTargetsLoading = ref(false)
const selectedVideoTarget = ref<FollowTargetEntry | null>(null)
const targetWorks = ref<VideoCard[]>([])
const targetWorksPage = ref(1)
const targetWorksHasNext = ref(false)
const targetWorksLoading = ref(false)
const targetWorksError = ref('')
// 同番号变体按需求 12 口径归并(中字>有码>无码),与目标作品页一致
const targetWorkItems = computed(() => dedupeByVideoBase(targetWorks.value).map((v) => fromVideo(v)))
const followedTargetIds = computed(() => new Set(videoFollows.value.map((f) => f.targetId)))
const targetWorksLink = computed(() => {
  const t = selectedVideoTarget.value
  return t ? `/videos/target/${VIDEO_SOURCE}/${videoKind.value}/${encodeURIComponent(t.targetId)}` : ''
})

// 请求序号:快速切换分组/对象时丢弃过期响应
let followsSeq = 0
let targetsSeq = 0
let worksSeq = 0

async function loadVideoFollows() {
  const seq = ++followsSeq
  const kind = videoKind.value
  videoFollowsLoading.value = true
  try {
    const list = await videoFollowList(VIDEO_SOURCE, kind)
    if (seq !== followsSeq) return
    videoFollows.value = list
    const keep = list.find((f) => f.targetId === selectedVideoTarget.value?.targetId)
    if (keep) selectedVideoTarget.value = keep
    else selectVideoTarget(list[0] ?? null)
  } catch (e) {
    if (seq === followsSeq) console.error('[following]', errorMessage(e))
  } finally {
    if (seq === followsSeq) videoFollowsLoading.value = false
  }
}

async function loadVideoTargets() {
  const seq = ++targetsSeq
  const kind = videoKind.value
  videoTargetsLoading.value = true
  try {
    const list = await videoFollowTargets(VIDEO_SOURCE, kind)
    if (seq === targetsSeq) videoTargets.value = list
  } catch (e) {
    if (seq === targetsSeq) console.error('[following]', errorMessage(e))
  } finally {
    if (seq === targetsSeq) videoTargetsLoading.value = false
  }
}

async function loadTargetWorks(reset = true) {
  const t = selectedVideoTarget.value
  if (!t || (!reset && targetWorksLoading.value)) return
  const seq = ++worksSeq
  const kind = videoKind.value
  const page = reset ? 1 : targetWorksPage.value + 1
  if (reset) {
    targetWorks.value = []
    targetWorksHasNext.value = false
  }
  targetWorksLoading.value = true
  targetWorksError.value = ''
  try {
    const res = await videoFollowWorks(VIDEO_SOURCE, kind, t.targetId, page)
    if (seq !== worksSeq) return
    if (reset) {
      targetWorks.value = res.items
    } else {
      const seen = new Set(targetWorks.value.map((v) => v.code))
      targetWorks.value.push(...res.items.filter((v) => !seen.has(v.code)))
    }
    targetWorksPage.value = page
    targetWorksHasNext.value = res.pagination.hasNext
  } catch (e) {
    if (seq === worksSeq) targetWorksError.value = errorMessage(e) || '作品加载失败'
  } finally {
    if (seq === worksSeq) targetWorksLoading.value = false
  }
}

function selectVideoTarget(entry: FollowTargetEntry | null) {
  if (entry && entry.targetId === selectedVideoTarget.value?.targetId) return
  selectedVideoTarget.value = entry
  if (entry) {
    void loadTargetWorks(true)
  } else {
    worksSeq++
    targetWorks.value = []
    targetWorksHasNext.value = false
    targetWorksLoading.value = false
    targetWorksError.value = ''
  }
}

/** 切换厂商/女优分组:清空上一分组的对象、作品与可关注目标后重拉 */
function switchVideoKind(kind: VideoKind) {
  if (kind === videoKind.value) return
  videoKind.value = kind
  videoFollows.value = []
  videoTargets.value = []
  selectVideoTarget(null)
  void loadVideoFollows()
  if (videoManage.value) void loadVideoTargets()
}

function toggleVideoManage() {
  videoManage.value = !videoManage.value
  if (videoManage.value && !videoTargets.value.length) void loadVideoTargets()
}

/** 乐观关注:立即标记「已关注」,失败回滚;成功后静默同步服务端列表 */
async function followVideoTarget(item: FollowTargetItem) {
  if (followedTargetIds.value.has(item.targetId)) return
  notice.value = ''
  const kind = videoKind.value
  const entry: FollowTargetEntry = {
    source: VIDEO_SOURCE,
    kind,
    targetId: item.targetId,
    displayName: item.displayName,
    followedAt: Date.now(),
  }
  videoFollows.value = [...videoFollows.value, entry]
  try {
    await videoFollow(VIDEO_SOURCE, kind, item.targetId, item.displayName)
    if (kind === videoKind.value) void loadVideoFollows()
  } catch (e) {
    if (kind === videoKind.value) {
      videoFollows.value = videoFollows.value.filter((f) => f.targetId !== item.targetId)
    }
    notice.value = `关注失败:${errorMessage(e)}`
  }
}

/** 乐观取关:先移除芯片(选中项顺延到第一个),失败回滚 */
async function unfollowVideoTarget(entry: FollowTargetEntry) {
  notice.value = ''
  const kind = videoKind.value
  const prev = videoFollows.value
  videoFollows.value = prev.filter((f) => f.targetId !== entry.targetId)
  if (selectedVideoTarget.value?.targetId === entry.targetId) {
    selectVideoTarget(videoFollows.value[0] ?? null)
  }
  try {
    await videoUnfollow(VIDEO_SOURCE, kind, entry.targetId)
  } catch (e) {
    if (kind === videoKind.value) videoFollows.value = prev
    notice.value = `取消关注失败:${errorMessage(e)}`
  }
}

// ============================================================
// 生命周期:首次加载、keep-alive 返回静默刷新、滚动恢复
// ============================================================
const refreshing = computed(() =>
  activeTab.value === 'authors' ? syncing.value : videoFollowsLoading.value || targetWorksLoading.value,
)

/** 顶部刷新:按当前视图分流 */
function handleRefresh() {
  notice.value = ''
  if (activeTab.value === 'authors') {
    void loadAllAuthors('refresh')
  } else {
    void loadVideoFollows()
    void loadTargetWorks(true)
    if (videoManage.value) void loadVideoTargets()
  }
}

onMounted(async () => {
  // 两组并行加载(首条出现时间优化)
  await Promise.all([loadFollows(), loadVideoFollows()])
  await loadAllAuthors('initial')
})

// 离开路由时 DOM 尚在,记录的 scrollY 可靠(deactivated 时页面已移出文档)
onBeforeRouteLeave(() => {
  tabScroll[activeTab.value] = window.scrollY
})

// 关注对象单行横滑:选中项(含首次/切换分组后)滚进可视区
const pageRoot = ref<HTMLElement | null>(null)
function revealActiveChips() {
  void nextTick(() => {
    const smooth = !window.matchMedia('(prefers-reduced-motion: reduce)').matches
    pageRoot.value?.querySelectorAll<HTMLElement>('.follow-chips .follow-chip.active').forEach((el) => {
      const strip = el.parentElement
      if (!strip) return
      const left = el.offsetLeft - (strip.clientWidth - el.offsetWidth) / 2
      strip.scrollTo({ left: Math.max(0, left), behavior: smooth ? 'smooth' : 'auto' })
    })
  })
}
watch([selectedComicAuthor, () => selectedVideoTarget.value?.targetId, activeTab], revealActiveChips)

// keep-alive 返回:恢复滚动 + 静默刷新(首次挂载也会触发 activated,跳过)
let activatedOnce = false
onActivated(async () => {
  if (!activatedOnce) {
    activatedOnce = true
    return
  }
  void nextTick(() => window.scrollTo(0, tabScroll[activeTab.value]))
  void loadVideoFollows()
  await loadFollows()
  const changed = authorsKey() !== loadedAuthorsKey
  if (changed || Date.now() - loadedAuthorsAt > STALE_MS) void loadAllAuthors('background')
})
</script>

<template>
  <div ref="pageRoot" class="following-page">
    <div class="container">
      <div class="page-heading">
        <h2 class="page-title">关注</h2>
        <button
          type="button"
          class="heading-btn"
          :disabled="refreshing"
          @click="handleRefresh"
        >
          {{ refreshing ? '刷新中' : '刷新' }}
        </button>
      </div>

      <div
        class="segmented"
        role="tablist"
        aria-label="关注视图"
        :style="{ '--seg-count': tabs.length, '--seg-index': activeTabIndex }"
      >
        <span class="segmented-thumb" aria-hidden="true" />
        <button
          v-for="tab in tabs"
          :key="tab.key"
          type="button"
          class="segmented-btn"
          :class="{ active: activeTab === tab.key }"
          role="tab"
          :aria-selected="activeTab === tab.key"
          @click="switchTab(tab.key)"
        >
          {{ tab.label }}
        </button>
      </div>

      <p v-if="notice" class="status-line is-error" role="alert">{{ notice }}</p>

      <!-- 漫画作者 -->
      <template v-if="activeTab === 'authors'">
        <LoadingSpinner v-if="!authorsReady" message="加载中..." />
        <EmptyState
          v-else-if="!followed.length"
          title="还没有关注任何作者"
          message="在漫画详情页点击作者旁的心形图标即可关注"
        />
        <template v-else>
          <p v-if="syncing && syncProgress" class="status-line">{{ syncProgress }}</p>
          <template v-else>
            <p v-if="syncError" class="status-line is-error">{{ syncError }}</p>
            <p v-if="bikaHint" class="status-line is-error">{{ bikaHint }}</p>
          </template>

          <TransitionGroup tag="div" name="chip" class="follow-chips" role="group" aria-label="关注的漫画作者">
            <div key="__all__" class="follow-chip" :class="{ active: selectedComicAuthor === null }">
              <button
                type="button"
                class="follow-chip-main"
                :aria-pressed="selectedComicAuthor === null"
                @click="selectedComicAuthor = null"
              >
                全部
              </button>
            </div>
            <div
              v-for="fa in followed"
              :key="fa.author"
              class="follow-chip"
              :class="{ active: selectedComicAuthor === fa.author, armed: armedKey === `a:${fa.author}` }"
            >
              <button
                type="button"
                class="follow-chip-main"
                :aria-pressed="selectedComicAuthor === fa.author"
                @click="selectedComicAuthor = fa.author"
              >
                {{ fa.author }}
              </button>
              <button
                type="button"
                class="follow-chip-remove"
                :aria-label="armedKey === `a:${fa.author}` ? `确认取消关注 ${fa.author}` : `取消关注 ${fa.author}`"
                @click="confirmTap(`a:${fa.author}`, () => unfollowAuthor(fa.author))"
              >
                <span v-if="armedKey === `a:${fa.author}`" class="follow-chip-confirm">取消关注</span>
                <span v-else aria-hidden="true">×</span>
              </button>
            </div>
          </TransitionGroup>

          <MediaCollection
            :items="allWorkItems"
            density="comic"
            :loading="syncing"
            :loading-more="loadingMore"
            :has-more="hasMoreWorks && !syncing"
            end-text="已加载全部作品"
            empty-title="暂无作品"
            empty-message="未能获取到关注作者的作品"
            @load-more="loadNextPage"
          />
        </template>
      </template>

      <!-- 视频:厂商/女优 -->
      <template v-else>
        <div class="toolbar-row">
          <div
            class="segmented is-compact"
            role="tablist"
            aria-label="视频关注分组"
            :style="{ '--seg-count': VIDEO_KINDS.length, '--seg-index': videoKindIndex }"
          >
            <span class="segmented-thumb" aria-hidden="true" />
            <button
              v-for="k in VIDEO_KINDS"
              :key="k.key"
              type="button"
              class="segmented-btn"
              :class="{ active: videoKind === k.key }"
              role="tab"
              :aria-selected="videoKind === k.key"
              @click="switchVideoKind(k.key)"
            >
              {{ k.label }}
            </button>
          </div>
          <button
            type="button"
            class="heading-btn"
            :class="{ 'is-accent': videoManage }"
            :aria-pressed="videoManage"
            @click="toggleVideoManage"
          >
            {{ videoManage ? '完成' : '管理关注' }}
          </button>
        </div>

        <div v-if="videoManage" class="manage-panel">
          <LoadingSpinner v-if="videoTargetsLoading && !videoTargets.length" message="加载可关注目标..." />
          <EmptyState
            v-else-if="!videoTargets.length"
            title="暂无可关注目标"
            message="该分类下没有解析到关注目标"
          />
          <ul v-else class="target-list">
            <li v-for="t in videoTargets" :key="t.targetId" class="target-row">
              <span class="target-name">{{ t.displayName }}</span>
              <span v-if="t.workCount" class="target-count">{{ formatCount(t.workCount) }} 部作品</span>
              <button
                type="button"
                class="target-follow-btn"
                :class="{ followed: followedTargetIds.has(t.targetId) }"
                :disabled="followedTargetIds.has(t.targetId)"
                @click="followVideoTarget(t)"
              >
                {{ followedTargetIds.has(t.targetId) ? '已关注' : '+ 关注' }}
              </button>
            </li>
          </ul>
        </div>

        <template v-else>
          <LoadingSpinner v-if="videoFollowsLoading && !videoFollows.length" message="加载关注列表..." />
          <EmptyState
            v-else-if="!videoFollows.length"
            title="还没有关注目标"
            message="点「管理关注」添加想跟踪的厂商或女优"
          />
          <TransitionGroup
            v-else
            tag="div"
            name="chip"
            class="follow-chips"
            role="group"
            aria-label="关注的厂商/女优"
          >
            <div
              v-for="f in videoFollows"
              :key="f.targetId"
              class="follow-chip"
              :class="{ active: selectedVideoTarget?.targetId === f.targetId, armed: armedKey === `v:${f.targetId}` }"
            >
              <button
                type="button"
                class="follow-chip-main"
                :aria-pressed="selectedVideoTarget?.targetId === f.targetId"
                @click="selectVideoTarget(f)"
              >
                {{ f.displayName }}
              </button>
              <button
                type="button"
                class="follow-chip-remove"
                :aria-label="armedKey === `v:${f.targetId}` ? `确认取消关注 ${f.displayName}` : `取消关注 ${f.displayName}`"
                @click="confirmTap(`v:${f.targetId}`, () => unfollowVideoTarget(f))"
              >
                <span v-if="armedKey === `v:${f.targetId}`" class="follow-chip-confirm">取消关注</span>
                <span v-else aria-hidden="true">×</span>
              </button>
            </div>
          </TransitionGroup>

          <template v-if="selectedVideoTarget">
            <div class="section-heading">
              <span class="section-title">{{ selectedVideoTarget.displayName }} 的作品</span>
              <button type="button" class="link-btn" @click="router.push(targetWorksLink)">
                查看全部 ›
              </button>
            </div>
            <MediaCollection
              :items="targetWorkItems"
              density="video"
              pager="button"
              :loading="targetWorksLoading && !targetWorks.length"
              :loading-more="targetWorksLoading && targetWorks.length > 0"
              :has-more="targetWorksHasNext"
              :error="targetWorksError"
              end-text=""
              empty-title="暂无作品"
              @load-more="loadTargetWorks(false)"
              @retry="loadTargetWorks(!targetWorks.length)"
            />
          </template>
        </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.following-page {
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
  border-color: var(--color-accent);
  color: var(--color-accent);
}

.heading-btn:disabled {
  opacity: 0.55;
}

/* —— 分段控件(关注/历史两页同款):滑块用 transform 平移 —— */
.segmented {
  position: relative;
  display: grid;
  grid-template-columns: repeat(var(--seg-count), minmax(0, 1fr));
  width: 100%;
  max-width: calc(var(--seg-count) * 112px);
  padding: 2px;
  margin-bottom: var(--spacing-lg);
  border-radius: var(--radius-pill);
  background: var(--color-tag-bg);
}

.segmented.is-compact {
  max-width: calc(var(--seg-count) * 80px);
  margin-bottom: 0;
}

.segmented-thumb {
  position: absolute;
  top: 2px;
  bottom: 2px;
  left: 2px;
  width: calc((100% - 4px) / var(--seg-count));
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  box-shadow: 0 0 0 1px var(--color-border);
  transform: translateX(calc(var(--seg-index) * 100%));
  transition: transform var(--duration-base) var(--ease-out);
}

.segmented-btn {
  position: relative;
  min-height: 40px;
  padding: 0 var(--spacing-sm);
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  white-space: nowrap;
  transition: color var(--duration-base) var(--ease-standard);
}

.segmented-btn.active {
  color: var(--color-text-primary);
}

.segmented-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}

/* —— 状态文案 —— */
.status-line {
  margin: calc(var(--spacing-sm) * -1) 0 var(--spacing-md);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.status-line.is-error {
  color: var(--color-error);
}

.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
}

/* —— 关注对象芯片(漫画作者 / 厂商·女优统一):点选看作品,× 二次确认取关 —— */
/* 单行横向滚动:关注再多也只占一行;上下留 4px 让 44px 命中区不被裁切,两端渐隐提示可滑动 */
.follow-chips {
  display: flex;
  flex-wrap: nowrap;
  gap: var(--spacing-xs);
  /* 出血到 .container 两侧内边距,滑动区贴屏幕边缘 */
  margin: calc(-1 * var(--spacing-xxs)) calc(-1 * var(--spacing-lg)) calc(var(--spacing-md) - var(--spacing-xxs));
  padding: var(--spacing-xxs) var(--spacing-lg);
  overflow-x: auto;
  overscroll-behavior-x: contain;
  scroll-snap-type: x proximity;
  scroll-padding-inline: var(--spacing-lg);
  scrollbar-width: none;
  -webkit-overflow-scrolling: touch;
  -webkit-mask-image: linear-gradient(to right, transparent, black var(--spacing-lg), black calc(100% - var(--spacing-lg)), transparent);
  mask-image: linear-gradient(to right, transparent, black var(--spacing-lg), black calc(100% - var(--spacing-lg)), transparent);
}

.follow-chips::-webkit-scrollbar {
  display: none;
}

.follow-chip {
  flex: 0 0 auto;
  max-width: 12em;
  scroll-snap-align: start;
  display: inline-flex;
  align-items: center;
  min-width: 0;
  min-height: 36px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard);
}

.follow-chip.active {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: var(--color-on-primary);
}

.follow-chip.armed {
  border-color: var(--color-error);
}

.follow-chip-main,
.follow-chip-remove {
  position: relative;
  align-self: stretch;
  color: inherit;
}

/* 视觉 36px,命中区扩到 44px(芯片行距 8px,上下各 4px 不重叠) */
.follow-chip-main::after,
.follow-chip-remove::after {
  content: '';
  position: absolute;
  inset: -4px 0;
}

.follow-chip-main {
  min-width: 0;
  padding: 0 var(--spacing-sm) 0 var(--spacing-md);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.follow-chip-main:only-child {
  padding-right: var(--spacing-md);
}

.follow-chip-remove {
  flex-shrink: 0;
  min-width: 32px;
  padding: 0 var(--spacing-sm) 0 var(--spacing-xxs);
  font-size: var(--font-size-body);
  line-height: 1;
  opacity: 0.6;
}

.follow-chip.armed .follow-chip-remove {
  opacity: 1;
  color: var(--color-error);
}

.follow-chip.armed.active .follow-chip-remove {
  color: var(--color-on-primary);
}

.follow-chip-confirm {
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
}

.chip-enter-active,
.chip-leave-active {
  transition:
    opacity var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.chip-enter-from,
.chip-leave-to {
  opacity: 0;
  transform: scale(0.9);
}

/* —— 管理关注面板 —— */
.manage-panel {
  max-height: 40vh;
  padding: var(--spacing-xs);
  margin-bottom: var(--spacing-md);
  overflow-y: auto;
  overscroll-behavior: contain;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
}

.target-list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.target-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  min-height: 44px;
  padding: 0 var(--spacing-sm);
  border-radius: var(--radius-sm);
}

.target-row + .target-row {
  border-top: 1px solid var(--color-divider-soft);
}

.target-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.target-count {
  flex-shrink: 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-fine-print);
}

.target-follow-btn {
  flex-shrink: 0;
  min-height: 32px;
  padding: 0 var(--spacing-sm);
  border-radius: var(--radius-pill);
  background: var(--color-accent);
  color: var(--color-on-primary);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard);
}

.target-follow-btn.followed {
  background: var(--color-tag-bg);
  color: var(--color-text-muted);
  cursor: default;
}

/* —— 作品预览区 —— */
.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-sm);
  margin: var(--spacing-lg) 0 var(--spacing-sm);
}

.section-title {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.link-btn {
  flex-shrink: 0;
  min-height: 44px;
  color: var(--color-accent);
  font-size: var(--font-size-caption);
}
</style>
