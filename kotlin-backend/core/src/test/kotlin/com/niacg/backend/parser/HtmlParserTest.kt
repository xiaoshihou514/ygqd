package com.niacg.backend.parser

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class HtmlParserTest {

    @Test
    fun `resolveUrl returns empty for blank input`() {
        assertEquals("", HtmlParser.resolveUrl(""))
        assertEquals("", HtmlParser.resolveUrl("   "))
    }

    @Test
    fun `resolveUrl does not modify absolute URLs`() {
        assertEquals("https://example.com/img.jpg", HtmlParser.resolveUrl("https://example.com/img.jpg"))
        assertEquals("http://example.com/img.jpg", HtmlParser.resolveUrl("http://example.com/img.jpg"))
    }

    @Test
    fun `resolveUrl prepends base for relative paths`() {
        assertEquals("https://www.niacg.com/uploads/img.jpg", HtmlParser.resolveUrl("/uploads/img.jpg"))
        assertEquals("https://www.niacg.com/uploads/img.jpg", HtmlParser.resolveUrl("uploads/img.jpg"))
    }

    @Test
    fun `parseHomepage extracts COS and CG sections`() {
        val html = loadResource("homepage.html")
        val sections = HtmlParser.parseHomepage(html)

        assertEquals(2, sections.size)

        val cosSection = sections.find { it.category == "COS" }
        assertTrue(cosSection != null)
        assertEquals(1, cosSection!!.categoryId)
        assertEquals(2, cosSection.items.size)
        assertEquals("COS Test Item", cosSection.items[0].title)
        assertEquals("520", cosSection.items[1].likes)

        val cgSection = sections.find { it.category == "CG" }
        assertTrue(cgSection != null)
        assertEquals(2, cgSection!!.categoryId)
        assertEquals(1, cgSection.items.size)
    }

    @Test
    fun `parseListPage extracts items and pagination`() {
        val html = loadResource("listpage.html")
        val result = HtmlParser.parseListPage(html)

        assertEquals(2, result.items.size)
        assertEquals("List Item 1", result.items[0].title)
        assertEquals("List Item 2", result.items[1].title)

        assertEquals(0, result.pagination.current)
        assertEquals(2, result.pagination.total)
        assertTrue(result.pagination.hasNext)
        assertFalse(result.pagination.hasPrev)
        assertEquals(0, result.pagination.current)
    }

    @Test
    fun `parseSearchResults extracts items and page template`() {
        val html = loadResource("search.html")
        val result = HtmlParser.parseSearchResults(html)

        assertEquals(2, result.items.size)
        assertEquals("Search Result 1", result.items[0].title)
        assertEquals("Search Result 2", result.items[1].title)
        assertTrue(result.pageUrlTemplate != null)
    }

    @Test
    fun `extractTagSearchMaxPage returns 0 for single page`() {
        val html = loadResource("tagsearch.html")
        val maxPage = HtmlParser.extractTagSearchMaxPage(html, "testkeyword")
        assertEquals(0, maxPage)
    }

    @Test
    fun `parseComicDetail extracts all fields`() {
        val html = loadResource("detail.html")
        val detail = HtmlParser.parseComicDetail(html, 9, "test-id")

        assertEquals("test-id", detail.id)
        assertEquals("Test Comic Detail", detail.title)
        assertEquals("A漫", detail.category)
        assertEquals(9, detail.categoryId)
        assertEquals("Author Name", detail.author)
        assertEquals(listOf("Work A", "Work B"), detail.works)
        assertEquals(listOf("Char 1", "Char 2"), detail.characters)
        assertEquals(listOf("tag1", "tag2"), detail.tags)
        assertEquals("9999", detail.likes)
        assertTrue(detail.thumbnail.contains("example.com"))
        assertEquals(0, detail.images.size)
    }

    @Test
    fun `parseComicImages extracts matching images`() {
        val html = loadResource("images.html")
        val images = HtmlParser.parseComicImages(html)

        assertEquals(3, images.size)
        assertTrue(images.any { it.contains("boom.example.com") })
        assertTrue(images.any { it.contains("xunge.example.com") })
        assertTrue(images.any { it.contains("hen.example.com") })
    }

    @Test
    fun `parseListAsSearchResult wraps list result`() {
        val html = loadResource("listpage.html")
        val result = HtmlParser.parseListAsSearchResult(html)

        assertEquals(2, result.items.size)
        assertEquals(0, result.pagination.current)
        assertFalse(result.pagination.hasNext)
    }

    @Test
    fun `parseSearchResults with no items attempts list fallback`() {
        val html = loadResource("listpage.html")
        val result = HtmlParser.parseSearchResults(html)

        assertEquals(2, result.items.size)
    }

    @Test
    fun `parseHomepage with empty HTML returns empty list`() {
        val sections = HtmlParser.parseHomepage("<html></html>")
        assertEquals(0, sections.size)
    }

    @Test
    fun `parseListPage with minimal HTML returns empty result`() {
        val result = HtmlParser.parseListPage("""<html><body><div></div></body></html>""")
        assertEquals(0, result.items.size)
    }

    @Test
    fun `parseComicDetail with missing tag-block handles gracefully`() {
        val html = """
            <html>
            <body>
            <h1>Only Title</h1>
            <div id="album_photo_cover"><img src="img.jpg" /></div>
            </body>
            </html>
        """.trimIndent()
        val detail = HtmlParser.parseComicDetail(html, 1, "123")
        assertEquals("Only Title", detail.title)
        assertEquals("", detail.author)
        assertEquals(0, detail.works.size)
        assertEquals(0, detail.characters.size)
        assertEquals(0, detail.tags.size)
    }

    @Test
    fun `parseComicImages with empty HTML returns empty list`() {
        val images = HtmlParser.parseComicImages("<html></html>")
        assertEquals(0, images.size)
    }

    @Test
    fun `parseListAsSearchResult with empty HTML returns empty result`() {
        val result = HtmlParser.parseListAsSearchResult("<html></html>")
        assertEquals(0, result.items.size)
    }

    @Test
    fun `extractTagSearchMaxPage with no pagination returns 0`() {
        val html = "<html></html>"
        val maxPage = HtmlParser.extractTagSearchMaxPage(html, "none")
        assertEquals(0, maxPage)
    }

    private fun loadResource(name: String): String {
        return HtmlParserTest::class.java.classLoader.getResource(name)?.readText()
            ?: throw IllegalArgumentException("Resource not found: $name")
    }
}
