import { computed, ref } from 'vue'
import { fetchFollowedAuthors, followAuthor as apiFollow, unfollowAuthor as apiUnfollow } from '@/services/api'
import type { FollowedAuthor } from '@/types'

const followed = ref<FollowedAuthor[]>([])
const loading = ref(false)
/** 并发拉取只落地最后一次发起的结果 */
let loadSeq = 0
let pendingLoads = 0
const followedSet = computed(() => new Set(followed.value.map((f) => f.author)))

async function fetchInto(showLoading: boolean): Promise<void> {
  const seq = ++loadSeq
  pendingLoads++
  if (showLoading) loading.value = true
  try {
    const list = await fetchFollowedAuthors()
    if (seq === loadSeq) followed.value = list
  } finally {
    pendingLoads--
    if (seq === loadSeq) loading.value = false
  }
}

function load(): Promise<void> {
  return fetchInto(true)
}

/** 关注/取关后本地更新列表(命令结果确定,列表按 followedAt 降序,新关注置顶),
 *  不翻 loading,页面不闪骨架;有拉取在途时改为静默重拉兜底(其结果可能早于本次写入) */
async function follow(author: string): Promise<void> {
  const added = await apiFollow(author)
  if (pendingLoads > 0) return fetchInto(false)
  if (!followedSet.value.has(added.author)) followed.value = [added, ...followed.value]
}

async function unfollow(author: string): Promise<void> {
  await apiUnfollow(author)
  if (pendingLoads > 0) return fetchInto(false)
  if (followedSet.value.has(author)) followed.value = followed.value.filter((f) => f.author !== author)
}

function isFollowing(author: string): boolean {
  return followedSet.value.has(author)
}

export function useFollowedAuthors() {
  return { followed, loading, load, follow, unfollow, isFollowing }
}
