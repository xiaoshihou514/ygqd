package com.niacg.backend.db

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ViewHistoryRepositoryTest {

    private val factory = DatabaseFactory.inMemory()
    private val repo = ViewHistoryRepository(factory)

    @Test
    fun `record adds entry to history`() {
        repo.record(entry("id1", "Title One", "AuthorA"))

        val list = repo.listRecent()
        assertEquals(1, list.size)
        assertEquals("id1", list[0].comicId)
        assertEquals("Title One", list[0].title)
        assertEquals("AuthorA", list[0].author)
    }

    @Test
    fun `record updates existing entry via upsert`() {
        repo.record(entry("id1", "Title One", "AuthorA"))
        Thread.sleep(5)
        repo.record(entry("id1", "Title One Updated", "AuthorB"))

        val list = repo.listRecent()
        assertEquals(1, list.size)
        assertEquals("Title One Updated", list[0].title)
        assertEquals("AuthorB", list[0].author)
    }

    @Test
    fun `listRecent respects limit`() {
        for (i in 1..10) {
            repo.record(entry("id$i", "Title $i", "Author"))
        }

        val list = repo.listRecent(5)
        assertEquals(5, list.size)
    }

    @Test
    fun `listRecent returns all when fewer than limit`() {
        repo.record(entry("id1", "Title", "Author"))

        val list = repo.listRecent(50)
        assertEquals(1, list.size)
    }

    @Test
    fun `listRecent returns most recent first`() {
        repo.record(entry("id1", "First", "A"))
        Thread.sleep(5)
        repo.record(entry("id2", "Second", "B"))

        val list = repo.listRecent()
        assertEquals("id2", list[0].comicId)
        assertEquals("id1", list[1].comicId)
    }

    @Test
    fun `listRecent returns empty list when no history`() {
        val list = repo.listRecent()
        assertEquals(0, list.size)
    }

    @Test
    fun `viewedAt is a timestamp`() {
        repo.record(entry("id1", "Title", "Author"))
        val entry = repo.listRecent()[0]
        assertTrue(entry.viewedAt > 0)
    }

    @Test
    fun `record stores all fields`() {
        repo.record(
            ViewHistoryEntry(
                comicId = "c1",
                title = "Test Title",
                thumbnail = "https://example.com/img.jpg",
                categoryId = 9,
                author = "TestAuthor",
                viewedAt = 1234567890L,
            )
        )

        val entry = repo.listRecent()[0]
        assertEquals("c1", entry.comicId)
        assertEquals("Test Title", entry.title)
        assertEquals("https://example.com/img.jpg", entry.thumbnail)
        assertEquals(9, entry.categoryId)
        assertEquals("TestAuthor", entry.author)
        assertEquals(1234567890L, entry.viewedAt)
    }

    @Test
    fun `default limit is 50`() {
        for (i in 1..60) {
            repo.record(entry("id$i", "Title $i", "A"))
        }

        val list = repo.listRecent()
        assertEquals(50, list.size)
    }

    companion object {
        private fun entry(id: String, title: String, author: String) = ViewHistoryEntry(
            comicId = id,
            title = title,
            thumbnail = "",
            categoryId = 9,
            author = author,
            viewedAt = System.currentTimeMillis(),
        )
    }
}
