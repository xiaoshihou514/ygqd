package com.niacg.backend.db

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class FollowedAuthorsRepositoryTest {

    private val factory = JdbcDatabaseProvider.inMemory()
    private val repo = FollowedAuthorsRepository(factory)

    @Test
    fun `follow adds author to list`() {
        repo.follow("AuthorA")

        val list = repo.listAll()
        assertEquals(1, list.size)
        assertEquals("AuthorA", list[0].author)
    }

    @Test
    fun `follow multiple authors returns in order`() {
        repo.follow("AuthorA")
        Thread.sleep(5)
        repo.follow("AuthorB")

        val list = repo.listAll()
        assertEquals(2, list.size)
        assertEquals("AuthorB", list[0].author)
        assertEquals("AuthorA", list[1].author)
    }

    @Test
    fun `duplicate follow does not create duplicate`() {
        repo.follow("AuthorA")
        repo.follow("AuthorA")

        val list = repo.listAll()
        assertEquals(1, list.size)
    }

    @Test
    fun `isFollowing returns true for followed author`() {
        repo.follow("AuthorA")
        assertTrue(repo.isFollowing("AuthorA"))
    }

    @Test
    fun `isFollowing returns false for unfollowed author`() {
        assertFalse(repo.isFollowing("NonExistent"))
    }

    @Test
    fun `unfollow removes author from list`() {
        repo.follow("AuthorA")
        repo.unfollow("AuthorA")

        val list = repo.listAll()
        assertEquals(0, list.size)
    }

    @Test
    fun `unfollow non-existent author does not throw`() {
        repo.unfollow("NonExistent")
        val list = repo.listAll()
        assertEquals(0, list.size)
    }

    @Test
    fun `touch updates lastCheckedAt`() {
        repo.follow("AuthorA")
        val before = repo.listAll()[0].lastCheckedAt

        Thread.sleep(5)
        repo.touch("AuthorA")

        val after = repo.listAll()[0].lastCheckedAt
        assertTrue(after > before)
    }

    @Test
    fun `listAll returns empty list when no authors followed`() {
        val list = repo.listAll()
        assertEquals(0, list.size)
    }

    @Test
    fun `followedAt is a timestamp`() {
        repo.follow("AuthorA")
        val author = repo.listAll()[0]
        assertTrue(author.followedAt > 0)
        assertTrue(author.lastCheckedAt > 0)
        assertEquals(author.followedAt, author.lastCheckedAt)
    }
}
