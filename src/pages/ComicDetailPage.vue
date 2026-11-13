<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { useInfiniteScroll } from '@/composables/useInfiniteScroll'
import { formatCount } from '@/utils/formatCount'
import { parseLikes } from '@/utils/likes'
import { ref, onMounted, onUnmounted, onActivated, onDeactivated, nextTick, computed } from 'vue'
import { useRoute, useRouter, onBeforeRouteUpdate, type RouteLocationNormalized } from 'vue-router'
import { fetchComicDetail, fetchComicImages, bikaSwitchLike, addFavorite, removeFavorite, isFavorite } from '@/services/api'
import type { ComicDetail } from '@/types'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useFollowedAuthors } from '@/composables/useFollowedAuthors'
import { useViewHistory } from '@/composables/useViewHistory'
import { extractAuthorFromTitle } from '@/utils/title'
import { parseSourceId, type SourceId } from '@/config/sources'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'
import ProxyImage from '@/components/ProxyImage.vue'

/** 每批挂载的图片数:ProxyImage 挂载即经 IPC 取二进制,小批量 = 天然并发上限 */
const PAGE_BATCH = 6
/** 预取距离:哨兵进入视口下方约两屏时挂载下一批(占位框有真实高度,不会连锁触发) */
const PREFETCH_MARGIN = '1200px'
/** 操作失败提示的停留时长 */
const ACTION_MSG_MS = 2600

const route = useRoute()
const router = useRouter()

const detail = ref<ComicDetail | null>(null)
const loading = ref(true)
const error = ref('')
const visibleCount = ref(PAGE_BATCH)
const images = ref<string[]>([])
const imagePage = ref(-1)
const imageHasNext = ref(false)
const imageLoading = ref(false)
const imageError = ref('')
const activeEp = ref(1)
/** 单页图片状态(按下标):已解码 / 失败;重试计数用于重挂 ProxyImage */
const loadedPages = ref(new Set<number>())
const failedPages = ref(new Set<number>())
const pageRetries = ref<Record<number, number>>({})
/** 当前作品源:由加载参数写入,不随失活后的全局路由漂移 */
const source = ref<SourceId>('niacg')
/** 哔咔服务器端点赞状态(仅 bika) */
const liked = ref(false)
/** 本地收藏状态(双源统一;哔咔另有服务器端状态,以本地为准展示) */
const favourited = ref(false)
const likeBusy = ref(false)
const favBusy = ref(false)
/** 关注操作进行中的乐观值(null = 无进行中操作,以 composable 列表为准) */
const followPending = ref<boolean | null>(null)
const actionMsg = ref('')
let actionMsgTimer = 0

const { add, has } = useTagBlacklist()
const { isFollowing, follow, unfollow, load: loadFollowed } = useFollowedAuthors()
const { record: recordHistory } = useViewHistory()

/** 竞态守卫:快速切换作品/章节时丢弃过期响应 */
let loadGen = 0
let imageGen = 0
let loadedKey = ''
let savedScrollY = 0

const episodeBar = ref<HTMLElement | null>(null)
const sentinel = ref<HTMLElement | null>(null)

const isBika = computed(() => source.value === 'bika')

const visibleImages = computed(() => images.value.slice(0, visibleCount.value))

const hasMore = computed(() =>
  visibleCount.value < images.value.length || imageHasNext.value,
)

const effectiveAuthor = computed(() => {
  if (!detail.value) return ''
  return detail.value.author || extractAuthorFromTitle(detail.value.title) || ''
})

const following = computed(() =>
  followPending.value ?? (effectiveAuthor.value ? isFollowing(effectiveAuthor.value) : false),
)

interface TagRow {
  key: string
  label: string
  items: string[]
  /** 点击标签跳搜索时的检索范围 */
  show: string
  banPrefix: string
}

const tagRows = computed<TagRow[]>(() => {
  const d = detail.value
  if (!d) return []
  const rows: TagRow[] = [
    { key: 'works', label: '作品', items: d.works, show: 'title,text', banPrefix: '拉黑作品标签' },
    { key: 'characters', label: '人物', items: d.characters, show: 'title,text,keyboard,ftitle', banPrefix: '拉黑人物标签' },
    { key: 'tags', label: '标签', items: d.tags, show: 'tags', banPrefix: '拉黑标签' },
  ]
  return rows.filter((r) => r.items.length > 0)
})

/** 下一章:order 大于当前的最小者(不假设章节序号连续/列表升序) */
const nextEp = computed(() => {
  if (!isBika.value || !detail.value) return undefined
  let best: number | undefined
  for (const ep of detail.value.episodes) {
    if (ep.order > activeEp.value && (best === undefined || ep.order < best)) best = ep.order
  }
  return best
})

const hasNextEp = computed(() => nextEp.value !== undefined)

function flashAction(msg: string) {
  actionMsg.value = msg
  clearTimeout(actionMsgTimer)
  actionMsgTimer = window.setTimeout(() => { actionMsg.value = '' }, ACTION_MSG_MS)
}

/** 哔咔点赞(toggle):乐观更新,失败回滚 */
async function toggleLike() {
  const d = detail.value
  if (!d || !isBika.value || likeBusy.value) return
  const prevLiked = liked.value
  const prevLikes = d.likes
  const next = !prevLiked
  liked.value = next
  d.likes = String(Math.max(0, parseLikes(prevLikes) + (next ? 1 : -1)))
  likeBusy.value = true
  try {
    await bikaSwitchLike(d.id)
  } catch (e) {
    if (detail.value === d) {
      liked.value = prevLiked
      d.likes = prevLikes
    }
    flashAction(`${next ? '点赞' : '取消点赞'}失败:${errorMessage(e)}`)
  } finally {
    likeBusy.value = false
  }
}

/** 收藏(双源):本地表为准,哔咔源同时同步服务器;乐观更新,失败回滚 */
async function toggleFavourite() {
  const d = detail.value
  if (!d || favBusy.value) return
  const next = !favourited.value
  favourited.value = next
  favBusy.value = true
  try {
    if (next) {
      await addFavorite({
        source: source.value,
        comicId: d.id,
        title: d.title,
        thumbnail: d.thumbnail,
        categoryId: d.categoryId,
        author: effectiveAuthor.value,
      })
    } else {
      await removeFavorite(source.value, d.id)
    }
  } catch (e) {
    if (detail.value === d) favourited.value = !next
    flashAction(`${next ? '收藏' : '取消收藏'}失败:${errorMessage(e)}`)
  } finally {
    favBusy.value = false
  }
}

/** 关注作者:乐观显示,失败回落到列表真实状态 */
async function toggleFollow() {
  const author = effectiveAuthor.value
  if (!author || followPending.value !== null) return
  const next = !following.value
  followPending.value = next
  try {
    await (next ? follow(author) : unfollow(author))
  } catch (e) {
    flashAction(`${next ? '关注' : '取消关注'}失败:${errorMessage(e)}`)
  } finally {
    followPending.value = null
  }
}

async function loadImagesPage(reset = false) {
  const d = detail.value
  if (!d) return
  if (!reset && (imageLoading.value || !imageHasNext.value)) return
  const gen = reset ? ++imageGen : imageGen
  const page = reset ? 0 : imagePage.value + 1
  const ep = activeEp.value
  imageLoading.value = true
  imageError.value = ''
  try {
    const result = await fetchComicImages(source.value, d.categoryId, d.id, ep, page)
    if (gen !== imageGen) return
    images.value = reset ? result.images : [...images.value, ...result.images]
    imagePage.value = page
    imageHasNext.value = result.pagination.hasNext
  } catch (e) {
    if (gen !== imageGen) return
    imageError.value = errorMessage(e)
  } finally {
    if (gen === imageGen) {
      imageLoading.value = false
      await nextTick()
      setupObserver()
    }
  }
}

/** 图片页加载失败后的手动重试(首页失败时走 reset 路径) */
function retryImages() {
  void loadImagesPage(imagePage.value < 0)
}

function loadMore() {
  if (visibleCount.value < images.value.length) {
    visibleCount.value = Math.min(visibleCount.value + PAGE_BATCH, images.value.length)
    void nextTick(setupObserver)
    return
  }
  if (imageHasNext.value) void loadImagesPage()
}

function resetGallery(hasNext: boolean) {
  ++imageGen
  imageLoading.value = false
  imageError.value = ''
  images.value = []
  visibleCount.value = PAGE_BATCH
  imagePage.value = -1
  imageHasNext.value = hasNext
  loadedPages.value = new Set()
  failedPages.value = new Set()
  pageRetries.value = {}
}

function onPageLoad(i: number) {
  if (!loadedPages.value.has(i)) loadedPages.value.add(i)
}

function onPageError(i: number) {
  failedPages.value.add(i)
}

function retryPage(i: number) {
  failedPages.value.delete(i)
  loadedPages.value.delete(i)
  pageRetries.value[i] = (pageRetries.value[i] ?? 0) + 1
}

function switchEp(order: number) {
  if (order === activeEp.value) return
  activeEp.value = order
  resetGallery(true)
  // 章节栏在视口上方时(如底部「下一章」)回到章节栏,而不是整页顶部
  const bar = episodeBar.value
  if (bar && bar.getBoundingClientRect().top < 0) bar.scrollIntoView({ block: 'start' })
  void loadImagesPage(true)
}

function goNextEp() {
  if (nextEp.value !== undefined) switchEp(nextEp.value)
}

const { setup: setupObserver, teardown: teardownObserver } = useInfiniteScroll({
  target: () => sentinel.value,
  onHit: () => loadMore(),
  // 出错时停绑,避免哨兵常驻视口导致失败请求被反复触发;由重试按钮恢复
  enabled: () => hasMore.value && !imageError.value,
  rootMargin: PREFETCH_MARGIN,
})

/** 本地收藏状态优先;哔咔若服务器已收藏(官方 app 操作)也视为已收藏 */
async function refreshFavourite(d: ComicDetail) {
  const serverFav = d.source === 'bika' && (d.isFavourite ?? false)
  try {
    const local = await isFavorite(d.source, d.id)
    if (detail.value === d && !favBusy.value) favourited.value = local || serverFav
  } catch {
    if (detail.value === d && !favBusy.value) favourited.value = serverFav
  }
}

function routeKey(r: RouteLocationNormalized): string {
  return `${String(r.params.source ?? '')}/${String(r.params.categoryId ?? '')}/${String(r.params.id ?? '')}`
}

async function loadComic(r: RouteLocationNormalized) {
  const gen = ++loadGen
  const src = parseSourceId((r.params.source as string) || 'niacg')
  const categoryId = Number(r.params.categoryId)
  const id = r.params.id as string
  loadedKey = routeKey(r)
  savedScrollY = 0
  window.scrollTo(0, 0)
  source.value = src
  loading.value = true
  error.value = ''
  actionMsg.value = ''
  detail.value = null
  liked.value = false
  favourited.value = false
  activeEp.value = 1
  resetGallery(false)
  teardownObserver()
  // 关注列表是全局单例,未进过关注页时为空,进详情时补拉一次以正确显示关注态
  loadFollowed().catch(() => {})

  try {
    const d = await fetchComicDetail(src, categoryId, id)
    if (gen !== loadGen) return
    detail.value = d
    liked.value = d.isLiked ?? false
    favourited.value = d.source === 'bika' && (d.isFavourite ?? false)
    recordHistory({
      source: d.source,
      comicId: d.id,
      title: d.title,
      thumbnail: d.thumbnail,
      categoryId: d.categoryId,
      author: effectiveAuthor.value,
    }).catch(() => {})
    void refreshFavourite(d)
    // 详情头先行展示,图片区自带加载态
    loading.value = false
    await loadImagesPage(true)
  } catch (e) {
    if (gen !== loadGen) return
    error.value = errorMessage(e)
    detail.value = null
  } finally {
    if (gen === loadGen) loading.value = false
  }
}

function reload() {
  void loadComic(route)
}

onMounted(() => {
  void loadComic(route)
})

onBeforeRouteUpdate((to) => {
  if (routeKey(to) !== loadedKey) void loadComic(to)
})

// keep-alive:同一作品返回时保留已加载内容并恢复阅读位置;换作品或上次失败才重新加载
onActivated(() => {
  if (routeKey(route) !== loadedKey || (error.value && !loading.value)) {
    void loadComic(route)
    return
  }
  const d = detail.value
  if (d) void refreshFavourite(d)
  const y = savedScrollY
  void nextTick(() => {
    window.scrollTo(0, y)
    setupObserver()
  })
})

onDeactivated(() => {
  savedScrollY = window.scrollY
  teardownObserver()
})

onUnmounted(() => {
  teardownObserver()
  clearTimeout(actionMsgTimer)
})

function goBack() {
  router.back()
}

function searchByScope(keyword: string, show: string = 'title,text,keyboard,ftitle') {
  router.push({ name: 'search', query: { keyword, show, source: source.value } })
}
</script>

<script lang="ts">
export default {
  name: 'ComicDetailPage',
  inheritAttrs: false,
}
</script>

<template>
  <div class="detail-page">
    <div class="container">
      <button class="back-btn" @click="goBack">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M19 12H5M12 19l-7-7 7-7" />
        </svg>
        返回
      </button>

      <div v-if="loading" class="detail-header detail-skeleton" aria-busy="true" aria-label="正在加载">
        <div class="cover-col">
          <div class="cover-wrap sk-block" />
        </div>
        <div class="info-col">
          <div class="sk-line sk-title" />
          <div class="sk-line sk-w60" />
          <div class="sk-chips">
            <div class="sk-chip" />
            <div class="sk-chip" />
            <div class="sk-chip" />
          </div>
          <div class="sk-line sk-w80" />
          <div class="sk-line sk-w40" />
        </div>
      </div>

      <div v-else-if="error" class="detail-error">
        <EmptyState title="加载失败" :message="error">
          <button type="button" class="pill-btn retry-btn" @click="reload">重试</button>
        </EmptyState>
      </div>

      <template v-else-if="detail">
        <div class="detail-header">
          <div class="cover-col">
            <div class="cover-wrap">
              <ProxyImage
                v-if="detail.thumbnail"
                :src="detail.thumbnail"
                :source="detail.source"
                :alt="detail.title"
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
                <svg width="16" height="16" viewBox="0 0 24 24" fill="var(--color-like)" aria-hidden="true">
                  <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
                </svg>
                {{ formatCount(parseLikes(detail.likes)) }}
              </span>
            </div>
            <div class="detail-actions">
              <button
                v-if="isBika"
                type="button"
                class="meta-action"
                :class="{ on: liked }"
                :aria-pressed="liked"
                :aria-busy="likeBusy"
                :title="liked ? '取消点赞' : '点赞'"
                @click="toggleLike"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" :fill="liked ? 'currentColor' : 'none'" stroke="currentColor" stroke-width="2" aria-hidden="true">
                  <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
                </svg>
                {{ liked ? '已赞' : '点赞' }}
              </button>
              <button
                type="button"
                class="meta-action"
                :class="{ on: favourited }"
                :aria-pressed="favourited"
                :aria-busy="favBusy"
                :title="favourited ? '取消收藏' : '收藏'"
                @click="toggleFavourite"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" :fill="favourited ? 'currentColor' : 'none'" stroke="currentColor" stroke-width="2" aria-hidden="true">
                  <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z" />
                </svg>
                {{ favourited ? '已收藏' : '收藏' }}
              </button>
            </div>
            <Transition name="fade">
              <p v-if="actionMsg" class="action-msg" role="status">{{ actionMsg }}</p>
            </Transition>

            <div v-if="effectiveAuthor" class="detail-tag-row">
              <span class="tag-label">作者</span>
              <div class="detail-tags">
                <span class="tag-chip">
                  <button type="button" class="tag-chip-main" @click="searchByScope(effectiveAuthor)">
                    {{ effectiveAuthor }}
                  </button>
                  <button
                    v-if="!has(effectiveAuthor)"
                    type="button"
                    class="tag-chip-side tag-chip-ban"
                    :title="`拉黑作者标签: ${effectiveAuthor}`"
                    :aria-label="`拉黑作者标签: ${effectiveAuthor}`"
                    @click="add(effectiveAuthor)"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                      <circle cx="12" cy="12" r="10" />
                      <path d="M4.93 4.93l14.14 14.14" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    class="tag-chip-side tag-chip-follow"
                    :class="{ following }"
                    :aria-pressed="following"
                    :title="following ? `取消关注 ${effectiveAuthor}` : `关注 ${effectiveAuthor}`"
                    :aria-label="following ? `取消关注 ${effectiveAuthor}` : `关注 ${effectiveAuthor}`"
                    @click="toggleFollow"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" :fill="following ? 'currentColor' : 'none'" :stroke="following ? 'none' : 'currentColor'" stroke-width="2" aria-hidden="true">
                      <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
                    </svg>
                  </button>
                </span>
              </div>
            </div>
            <div v-for="row in tagRows" :key="row.key" class="detail-tag-row">
              <span class="tag-label">{{ row.label }}</span>
              <div class="detail-tags">
                <span v-for="tag in row.items" :key="tag" class="tag-chip">
                  <button type="button" class="tag-chip-main" @click="searchByScope(tag, row.show)">
                    {{ tag }}
                  </button>
                  <button
                    v-if="!has(tag)"
                    type="button"
                    class="tag-chip-side tag-chip-ban"
                    :title="`${row.banPrefix}: ${tag}`"
                    :aria-label="`${row.banPrefix}: ${tag}`"
                    @click="add(tag)"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                      <circle cx="12" cy="12" r="10" />
                      <path d="M4.93 4.93l14.14 14.14" />
                    </svg>
                  </button>
                </span>
              </div>
            </div>
            <div class="detail-count">
              {{ isBika ? `已加载 ${images.length} 张图片` : `共 ${images.length} 张图片` }}
            </div>
          </div>
        </div>

        <div v-if="isBika && detail.episodes.length > 0" ref="episodeBar" class="episode-bar" role="tablist" aria-label="章节">
          <button
            v-for="ep in detail.episodes"
            :key="ep.order"
            type="button"
            class="episode-pill"
            :class="{ active: ep.order === activeEp }"
            role="tab"
            :aria-selected="ep.order === activeEp"
            @click="switchEp(ep.order)"
          >
            {{ ep.title || `#${ep.order}` }}
          </button>
        </div>

        <div v-if="images.length > 0" class="detail-gallery">
          <div
            v-for="(img, i) in visibleImages"
            :key="`${activeEp}-${i}`"
            class="page-frame"
            :class="{ 'is-loaded': loadedPages.has(i), 'is-failed': failedPages.has(i) }"
            @load.capture="onPageLoad(i)"
          >
            <ProxyImage
              :key="pageRetries[i] ?? 0"
              :src="img"
              :source="detail.source"
              :alt="`${detail.title} - 第${i + 1}页`"
              loading="lazy"
              class="gallery-img"
              @error="onPageError(i)"
            />
            <button v-if="failedPages.has(i)" type="button" class="pill-btn page-retry" @click="retryPage(i)">
              重新加载第 {{ i + 1 }} 页
            </button>
          </div>

          <div v-show="hasMore" ref="sentinel" class="scroll-sentinel" />
          <div v-if="imageLoading" class="loading-more">
            <LoadingSpinner message="正在加载图片..." />
          </div>
          <div v-if="imageError" class="image-error" role="alert">
            <p>{{ imageError }}</p>
            <button type="button" class="pill-btn retry-btn" @click="retryImages">重试</button>
          </div>
          <div v-if="!hasMore && !hasNextEp" class="no-more">
            <span>已加载全部 {{ images.length }} 张图片</span>
          </div>
          <button v-if="hasNextEp && !imageLoading" type="button" class="next-ep-btn" @click="goNextEp">
            下一章
          </button>
        </div>

        <div v-else-if="imageLoading" class="detail-loading">
          <LoadingSpinner message="正在加载图片..." />
        </div>

        <div v-else-if="imageError" class="detail-empty">
          <EmptyState title="图片加载失败" :message="imageError">
            <button type="button" class="pill-btn retry-btn" @click="retryImages">重试</button>
          </EmptyState>
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
  gap: var(--spacing-xxs);
  min-height: 44px;
  padding: var(--spacing-xs) var(--spacing-md);
  margin-bottom: var(--spacing-md);
  color: var(--color-text-secondary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  background: var(--color-card-bg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

/* ---- 统一按压反馈 ---- */
.back-btn:active,
.meta-action:active,
.tag-chip-main:active,
.tag-chip-side:active,
.episode-pill:active,
.pill-btn:active,
.next-ep-btn:active {
  transform: scale(var(--press-scale));
}

.detail-loading,
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
  aspect-ratio: 5 / 7;
  overflow: hidden;
  border-radius: var(--radius-lg);
  background: var(--color-skeleton);
}

.cover-wrap img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  animation: fade-in var(--duration-base) var(--ease-out);
}

.cover-placeholder {
  position: absolute;
  inset: 0;
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
  line-height: var(--line-height-heading);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-sm);
  overflow-wrap: anywhere;
}

.detail-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--spacing-sm);
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
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-xxs);
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
  font-variant-numeric: tabular-nums;
}

.detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-sm);
}

.meta-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 40px;
  min-width: 88px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-secondary);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    background-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.meta-action.on {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.meta-action[aria-busy='true'] {
  cursor: progress;
}

.action-msg {
  margin: calc(-1 * var(--spacing-xxs)) 0 var(--spacing-sm);
  font-size: var(--font-size-fine-print);
  color: var(--color-error);
}

.detail-tag-row {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xs);
}

.tag-label {
  flex-shrink: 0;
  min-width: 36px;
  line-height: 32px;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-muted);
}

.detail-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
  min-width: 0;
}

/* ---- 分段标签:主体(跳搜索) + 侧按钮(拉黑/关注) ---- */
.tag-chip {
  display: inline-flex;
  align-items: stretch;
  max-width: 100%;
  min-height: 32px;
  border-radius: var(--radius-sm);
  overflow: hidden;
  background: var(--color-tag-bg);
}

.tag-chip-main {
  min-width: 0;
  padding: 0 10px;
  color: var(--color-tag-text);
  font-size: var(--font-size-caption);
  background: transparent;
  border: none;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition:
    color var(--duration-fast) var(--ease-standard),
    background-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.tag-chip-side {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  min-width: 32px;
  padding: 0 var(--spacing-xs);
  color: var(--color-text-muted);
  background: transparent;
  border: none;
  border-left: 1px solid var(--color-hairline);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease-standard),
    background-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.tag-chip-follow.following {
  background: var(--color-like);
  color: var(--color-on-error);
}

@media (hover: hover) {
  .back-btn:hover {
    color: var(--color-primary);
    border-color: var(--color-primary);
  }

  .meta-action:hover {
    border-color: var(--color-primary);
  }

  .tag-chip-main:hover {
    background: var(--color-primary);
    color: var(--color-on-primary);
  }

  .tag-chip-ban:hover {
    background: var(--color-error);
    color: var(--color-on-error);
  }

  .tag-chip-follow:hover {
    background: var(--color-like);
    color: var(--color-on-error);
  }

  .episode-pill:not(.active):hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
}

.detail-count {
  margin-top: var(--spacing-xxs);
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

/* ---- 章节 ---- */
.episode-bar {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-lg);
  scroll-margin-top: calc(var(--nav-total-height) + var(--spacing-sm));
}

.episode-pill {
  min-height: 36px;
  padding: 0 14px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  background: var(--color-card-bg);
  font-size: var(--font-size-fine-print);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    background-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.episode-pill.active {
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-color: var(--color-primary);
}

/* ---- 阅读区:每页一个稳定占位框,解码后才收拢为图片真实尺寸 ---- */
.detail-gallery {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-sm);
}

.page-frame {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  max-width: 100%;
}

.page-frame:not(.is-loaded) {
  width: 100%;
  aspect-ratio: 5 / 7;
  max-height: 90vh;
  overflow: hidden;
  border-radius: var(--radius-md);
  background: var(--color-skeleton);
  animation: skeleton-pulse calc(var(--duration-slow) * 4) var(--ease-standard) infinite alternate;
}

.page-frame.is-failed {
  aspect-ratio: auto;
  min-height: 200px;
  justify-content: center;
  gap: var(--spacing-sm);
  animation: none;
}

.gallery-img {
  max-width: 100%;
  max-height: 90vh;
  border-radius: var(--radius-md);
  opacity: 0;
  transition: opacity var(--duration-base) var(--ease-out);
}

.page-frame.is-loaded .gallery-img,
.page-frame.is-failed .gallery-img {
  opacity: 1;
}

.page-frame.is-failed .gallery-img {
  height: auto;
}

.scroll-sentinel {
  height: 1px;
  width: 100%;
}

.loading-more {
  padding: var(--spacing-lg) 0;
}

.image-error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-xs);
  padding: var(--spacing-sm) 0;
  font-size: var(--font-size-caption);
  color: var(--color-error);
  text-align: center;
}

.no-more {
  text-align: center;
  padding: var(--spacing-lg) 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

/* ---- 次级胶囊按钮(重试等,与 MediaCollection 重试一致) ---- */
.pill-btn {
  min-height: 40px;
  padding: 0 22px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
  transition:
    border-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.retry-btn {
  margin-top: var(--spacing-md);
}

.image-error .retry-btn {
  margin-top: 0;
}

.next-ep-btn {
  min-height: 44px;
  margin-bottom: var(--spacing-lg);
  padding: 0 var(--spacing-xl);
  border: none;
  border-radius: var(--radius-pill);
  color: var(--color-on-primary);
  background: var(--color-primary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
  transition: transform var(--duration-fast) var(--ease-standard);
}

.detail-empty {
  padding-top: var(--spacing-lg);
}

/* ---- 详情头骨架 ---- */
.detail-skeleton .sk-block,
.sk-line,
.sk-chip {
  background: var(--color-skeleton);
  animation: skeleton-pulse calc(var(--duration-slow) * 4) var(--ease-standard) infinite alternate;
}

.sk-line {
  height: 14px;
  margin-bottom: var(--spacing-sm);
  border-radius: var(--radius-xs);
}

.sk-title {
  height: 26px;
  width: 85%;
}

.sk-w80 { width: 80%; }
.sk-w60 { width: 60%; }
.sk-w40 { width: 40%; }

.sk-chips {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-sm);
}

.sk-chip {
  width: 88px;
  height: 40px;
  border-radius: var(--radius-pill);
}

/* ---- 过渡 ---- */
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--duration-base) var(--ease-standard);
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@keyframes skeleton-pulse {
  from { opacity: 1; }
  to { opacity: 0.55; }
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
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
