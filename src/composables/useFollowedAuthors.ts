import { computed, ref } from 'vue'
import { fetchFollowedAuthors, followAuthor as apiFollow, unfollowAuthor as apiUnfollow } from '@/services/api'
import type { FollowedAuthor } from '@/types'

const followed = ref<FollowedAuthor[]>([])
const loading = ref(false)
/** 并发 load(关注/取关后各自重拉)只落地最后一次发起的结果 */
let loadSeq = 0
const followedSet = computed(() => new Set(followed.value.map((f) => f.author)))

async function load(): Promise<void> {
  const seq = ++loadSeq
  loading.value = true
  try {
    const list = await fetchFollowedAuthors()
    if (seq === loadSeq) followed.value = list
  } finally {
    if (seq === loadSeq) loading.value = false
  }
}

export function useFollowedAuthors() {
  async function follow(author: string): Promise<void> {
    await apiFollow(author)
    await load()
  }

  async function unfollow(author: string): Promise<void> {
    await apiUnfollow(author)
    await load()
  }

  function isFollowing(author: string): boolean {
    return followedSet.value.has(author)
  }

  return { followed, loading, load, follow, unfollow, isFollowing }
}
