package com.niacg.backend.models

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class ModelsTest {

    @Test
    fun `ComicItem equality and hashCode`() {
        val a = ComicItem("1", "Title", "thumb", "CG", 2, listOf("a", "b"), "100", "/link")
        val b = ComicItem("1", "Title", "thumb", "CG", 2, listOf("a", "b"), "100", "/link")
        val c = ComicItem("2", "Title", "thumb", "CG", 2, listOf("a", "b"), "100", "/link")

        assertEquals(a, b)
        assertNotEquals(a, c)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `ComicItem toString`() {
        val item = ComicItem("1", "Title", "thumb", "CG", 2, listOf("a"), "100", "/link")
        val str = item.toString()
        assertTrue(str.contains("ComicItem"))
        assertTrue(str.contains("Title"))
    }

    @Test
    fun `ComicItem properties`() {
        val item = ComicItem("1", "Title", "thumb", "CG", 2, listOf("a"), "100", "/link")
        assertEquals("1", item.id)
        assertEquals("Title", item.title)
        assertEquals("thumb", item.thumbnail)
        assertEquals("CG", item.category)
        assertEquals(2, item.categoryId)
        assertEquals(listOf("a"), item.tags)
        assertEquals("100", item.likes)
        assertEquals("/link", item.link)
    }

    @Test
    fun `ComicItem componentN`() {
        val item = ComicItem("1", "Title", "thumb", "CG", 2, listOf("a"), "100", "/link")
        val (id, title, thumbnail, category, categoryId, tags, likes, link) = item
        assertEquals("1", id)
        assertEquals("Title", title)
        assertEquals("CG", category)
        assertEquals(2, categoryId)
    }

    @Test
    fun `ComicItem copy`() {
        val item = ComicItem("1", "Title", "thumb", "CG", 2, listOf("a"), "100", "/link")
        val copied = item.copy(title = "New", likes = "200")
        assertEquals("New", copied.title)
        assertEquals("200", copied.likes)
        assertEquals("1", copied.id)
    }

    @Test
    fun `ComicDetail equality`() {
        val a = ComicDetail("1", "Title", "thumb", "CG", 2, "author", listOf("w"), listOf("c"), listOf("t"), "200", listOf("img"))
        val b = ComicDetail("1", "Title", "thumb", "CG", 2, "author", listOf("w"), listOf("c"), listOf("t"), "200", listOf("img"))
        val c = a.copy(id = "2")

        assertEquals(a, b)
        assertNotEquals(a, c)
    }

    @Test
    fun `ComicDetail toString`() {
        val detail = ComicDetail("1", "T", "td", "CG", 2, "a", listOf(), listOf(), listOf(), "0", listOf())
        val str = detail.toString()
        assertTrue(str.contains("ComicDetail"))
        assertTrue(str.contains("T"))
    }

    @Test
    fun `ComicDetail properties`() {
        val detail = ComicDetail("1", "T", "thumb", "CG", 2, "author", listOf("w"), listOf("c"), listOf("t"), "200", listOf("i"))
        assertEquals("1", detail.id)
        assertEquals("T", detail.title)
        assertEquals("author", detail.author)
        assertEquals(listOf("w"), detail.works)
        assertEquals(listOf("c"), detail.characters)
        assertEquals(listOf("t"), detail.tags)
        assertEquals(listOf("i"), detail.images)
        assertEquals("200", detail.likes)
    }

    @Test
    fun `ComicDetail componentN`() {
        val d = ComicDetail("1", "T", "td", "CG", 2, "a", listOf("w"), listOf("c"), listOf("t"), "100", listOf("i"))
        val (id, title, _, _, _, author, works, chars, tags, likes, images) = d
        assertEquals("1", id)
        assertEquals("a", author)
        assertEquals(listOf("w"), works)
        assertEquals(listOf("c"), chars)
        assertEquals(listOf("t"), tags)
        assertEquals("100", likes)
        assertEquals(listOf("i"), images)
    }

    @Test
    fun `ComicDetail copy`() {
        val d = ComicDetail("1", "T", "td", "CG", 2, "a", listOf(), listOf(), listOf(), "0", listOf())
        val copied = d.copy(title = "New", likes = "999")
        assertEquals("New", copied.title)
        assertEquals("999", copied.likes)
    }

    @Test
    fun `HomeSection properties`() {
        val items = listOf(ComicItem("1", "T", "td", "CG", 2, emptyList(), "0", "/l"))
        val section = HomeSection("CG", 2, "CG推荐", items)
        assertEquals("CG", section.category)
        assertEquals(2, section.categoryId)
        assertEquals("CG推荐", section.label)
        assertEquals(1, section.items.size)
    }

    @Test
    fun `HomeSection toString`() {
        val items = listOf(ComicItem("1", "T", "td", "CG", 2, emptyList(), "0", "/l"))
        val section = HomeSection("CG", 2, "CG推荐", items)
        assertTrue(section.toString().contains("CG"))
    }

    @Test
    fun `CategoryListResult with pagination`() {
        val items = listOf(ComicItem("1", "T", "td", "CG", 2, emptyList(), "0", "/l"))
        val pagination = PaginationInfo(0, 3, true, false)
        val result = CategoryListResult(items, pagination)
        assertEquals(1, result.items.size)
        assertEquals(0, result.pagination.current)
        assertEquals(3, result.pagination.total)
        assertTrue(result.pagination.hasNext)
        assertFalse(result.pagination.hasPrev)
    }

    @Test
    fun `SearchResult properties`() {
        val items = listOf(ComicItem("1", "T", "td", "CG", 2, emptyList(), "0", "/l"))
        val pagination = PaginationInfo(1, 5, false, true)
        val result = SearchResult(items, pagination)
        assertEquals(1, result.items.size)
        assertEquals(5, result.pagination.total)
    }

    @Test
    fun `SearchResult toString`() {
        val result = SearchResult(emptyList(), PaginationInfo(0, 0, false, false))
        assertTrue(result.toString().contains("SearchResult"))
    }

    @Test
    fun `PaginationInfo with all true`() {
        val p = PaginationInfo(2, 5, true, true)
        assertTrue(p.hasNext)
        assertTrue(p.hasPrev)
        assertEquals(2, p.current)
    }

    @Test
    fun `PaginationInfo toString`() {
        val p = PaginationInfo(2, 5, true, true)
        assertTrue(p.toString().contains("Pagination"))
    }

    @Test
    fun `PaginationInfo equality`() {
        val a = PaginationInfo(0, 3, true, false)
        val b = PaginationInfo(0, 3, true, false)
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `PaginationInfo copy`() {
        val p = PaginationInfo(1, 3, false, false)
        val copied = p.copy(current = 2, hasPrev = true)
        assertEquals(2, copied.current)
        assertTrue(copied.hasPrev)
    }

    @Test
    fun `ApiResponse with data`() {
        val response = ApiResponse(200, "ok", "success")
        assertEquals(200, response.code)
        assertEquals("ok", response.data)
        assertEquals("success", response.message)
    }

    @Test
    fun `ApiResponse null data`() {
        val response = ApiResponse<Any>(500, null, "error")
        assertEquals(500, response.code)
        assertEquals(null, response.data)
        assertEquals("error", response.message)
    }

    @Test
    fun `ApiResponse equality`() {
        val a = ApiResponse(200, "data", null)
        val b = ApiResponse(200, "data", null)
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `ApiResponse copy`() {
        val response = ApiResponse(200, "data", null)
        val copied = response.copy(code = 404, data = "not found")
        assertEquals(404, copied.code)
        assertEquals("not found", copied.data)
        assertEquals(null, copied.message)
    }

    @Test
    fun `ApiResponse toString`() {
        val response = ApiResponse(200, "ok", null)
        assertTrue(response.toString().contains("ApiResponse"))
        assertTrue(response.toString().contains("ok"))
    }

    @Test
    fun `ApiResponse componentN`() {
        val response = ApiResponse(200, "data", "msg")
        val (code, data, message) = response
        assertEquals(200, code)
        assertEquals("data", data)
        assertEquals("msg", message)
    }

    @Test
    fun `SearchParams properties`() {
        val params = SearchParams("test", 3, "custom", "2")
        assertEquals("test", params.keyword)
        assertEquals(3, params.classid)
        assertEquals("custom", params.show)
        assertEquals("2", params.tempid)
    }

    @Test
    fun `SearchParams defaults`() {
        val params = SearchParams("keyword", 1)
        assertEquals("keyword", params.keyword)
        assertEquals(1, params.classid)
        assertEquals("title,text,keyboard,ftitle", params.show)
        assertEquals("1", params.tempid)
    }

    @Test
    fun `SearchParams equality`() {
        val a = SearchParams("k", 1, "s", "t")
        val b = SearchParams("k", 1, "s", "t")
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `SearchParams copy`() {
        val params = SearchParams("k", 1, "s", "t")
        val copied = params.copy(keyword = "new")
        assertEquals("new", copied.keyword)
        assertEquals(1, copied.classid)
    }

    @Test
    fun `SearchParams toString`() {
        val params = SearchParams("test", 2)
        assertTrue(params.toString().contains("SearchParams"))
    }

    @Test
    fun `SearchParams componentN`() {
        val params = SearchParams("k", 1, "s", "t")
        val (keyword, classid, show, tempid) = params
        assertEquals("k", keyword)
        assertEquals(1, classid)
        assertEquals("s", show)
        assertEquals("t", tempid)
    }

    @Test
    fun `CategoryOption properties`() {
        val option = CategoryOption(5, "CG插画")
        assertEquals(5, option.id)
        assertEquals("CG插画", option.label)
    }

    @Test
    fun `CategoryOption equality`() {
        val a = CategoryOption(1, "A")
        val b = CategoryOption(1, "A")
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `CategoryOption copy`() {
        val option = CategoryOption(1, "A")
        val copied = option.copy(label = "B")
        assertEquals(1, copied.id)
        assertEquals("B", copied.label)
    }

    @Test
    fun `CategoryOption toString`() {
        val option = CategoryOption(3, "Test")
        assertTrue(option.toString().contains("CategoryOption"))
    }

    @Test
    fun `CategoryOption componentN`() {
        val option = CategoryOption(7, "Label")
        val (id, label) = option
        assertEquals(7, id)
        assertEquals("Label", label)
    }
}
