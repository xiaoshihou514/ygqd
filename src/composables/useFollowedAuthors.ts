import { ref } from 'vue'
import { fetchFollowedAuthors, followAuthor as apiFollow, unfollowAuthor as apiUnfollow } from '@/services/api'
import type { FollowedAuthor } from '@/types'

const followed = ref<FollowedAuthor[]>([])
const loading = ref(false)

export function useFollowedAuthors() {
  async function load(): Promise<void> {
    loading.value = true
    try {
      followed.value = await fetchFollowedAuthors()
    } finally {
      loading.value = false
    }
  }

  async function follow(author: string): Promise<void> {
    await apiFollow(author)
    await load()
  }

  async function unfollow(author: string): Promise<void> {
    await apiUnfollow(author)
    await load()
  }

  function isFollowing(author: string): boolean {
    return followed.value.some((f) => f.author === author)
  }

  return { followed, loading, load, follow, unfollow, isFollowing }
}
