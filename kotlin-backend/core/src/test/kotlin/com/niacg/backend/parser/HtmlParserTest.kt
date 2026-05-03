package com.niacg.backend.parser

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class HtmlParserTest {

    @Test
    fun testResolveUrl() {
        assertEquals("", HtmlParser.resolveUrl(""))
        assertEquals("https://www.niacg.com/image.jpg", HtmlParser.resolveUrl("https://www.niacg.com/image.jpg"))
        assertEquals("/api/image.jpg", HtmlParser.resolveUrl("/api/image.jpg").takeLast(14))
    }

    @Test
    fun testDecodeHtmlEntities() {
        val html = "<div class=\"owl-item\"><div class=\"p-b-15\">" +
            """<a href="moehome-3-12345.html">""" +
            """<img data-src="https://example.com/thumb.jpg">""" +
            """<span class="video-title">测试标题 &amp; 更多</span>""" +
            """<span class="tag">标签1</span>""" +
            """<span class="label-category">本子</span>""" +
            """</a></div></div>"""

        val result = HtmlParser.parseListPage(html)
        assertTrue(result.items.isNotEmpty(), "Should find at least one item")
        assertEquals("12345", result.items[0].id)
        assertEquals("测试标题 & 更多", result.items[0].title)
        assertEquals("本子", result.items[0].category)
    }

    @Test
    fun testParseHomepageEmpty() {
        val html = "<html><body></body></html>"
        val result = HtmlParser.parseHomepage(html)
        assertEquals(0, result.size)
    }

    @Test
    fun testParseListPageEmpty() {
        val html = "<html><body></body></html>"
        val result = HtmlParser.parseListPage(html)
        assertEquals(0, result.items.size)
    }
}
