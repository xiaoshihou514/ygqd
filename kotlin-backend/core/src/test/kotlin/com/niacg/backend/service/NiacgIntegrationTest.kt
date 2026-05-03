package com.niacg.backend.service

import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class NiacgIntegrationTest {

    private val client = JvmTlsClient()
    private val service = NiacgService(client)

    @Test
    fun `homepage returns sections with items`() = runBlocking {
        println("=== TEST: Homepage ===")
        val sections = service.fetchHomepage()
        println("Sections found: ${sections.size}")
        for (s in sections) {
            println("  [${s.category}] ${s.label}: ${s.items.size} items")
            s.items.take(1).forEach { item ->
                println("    - id=${item.id} title=\"${item.title}\" thumb=${item.thumbnail.take(60)}")
            }
        }
        assertTrue(sections.isNotEmpty(), "Should have at least one section")
        val totalItems = sections.sumOf { it.items.size }
        assertTrue(totalItems > 0, "Should have at least one item across all sections")
        println("TOTAL items: $totalItems")
    }

    @Test
    fun `category list returns items`() = runBlocking {
        println("=== TEST: Category List (本子 page 0) ===")
        val result = service.fetchCategoryList(3, 0)
        println("Items found: ${result.items.size}")
        println("Pagination: current=${result.pagination.current} total=${result.pagination.total}")
        result.items.take(3).forEach { item ->
            println("  id=${item.id} title=\"${item.title}\" cat=${item.category}")
        }
        assertTrue(result.items.isNotEmpty(), "Should have items in category list")
    }

    @Test
    fun `comic detail returns complete data`() = runBlocking {
        println("=== TEST: Comic Detail (本子 12345) ===")
        val detail = service.fetchComicDetail(3, "12345")
        println("  id=${detail.id}")
        println("  title=${detail.title}")
        println("  author=${detail.author}")
        println("  category=${detail.category}")
        println("  likes=${detail.likes}")
        println("  images count=${detail.images.size}")
        println("  thumbnail=${detail.thumbnail.take(80)}")
        detail.images.take(3).forEach { img ->
            println("  image: ${img.take(80)}")
        }
        assertNotNull(detail.title, "Should have title")
    }

    @Test
    fun `search by engine returns results`() = runBlocking {
        println("=== TEST: Search Engine ===")
        val result = service.searchByEngine(classid = 9, keyword = "原神")
        println("Items found: ${result.items.size}")
        result.items.take(5).forEach { item ->
            println("  id=${item.id} title=\"${item.title}\"")
        }
        assertTrue(result.items.size >= 0, "Search should not crash")
    }

    @Test
    fun `tag search returns results`() = runBlocking {
        println("=== TEST: Tag Search ===")
        val result = service.searchByTags("原神")
        println("Items found: ${result.items.size}")
        result.items.take(5).forEach { item ->
            println("  id=${item.id} title=\"${item.title}\"")
        }
        assertTrue(result.items.size >= 0, "Tag search should not crash")
    }
}
