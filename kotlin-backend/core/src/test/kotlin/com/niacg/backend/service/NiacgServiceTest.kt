package com.niacg.backend.service

import com.niacg.backend.models.ComicItem
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class NiacgServiceTest {

    private val mockHtmlClient = object : HttpClient {
        override suspend fun get(path: String, cookies: List<String>): HttpClientResponse {
            return when {
                path == "/" -> HttpClientResponse(
                    status = 200,
                    body = SAMPLE_HOMEPAGE,
                    headers = emptyMap(),
                    setCookies = emptyList()
                )
                path.startsWith("/listinfo") -> HttpClientResponse(
                    status = 200,
                    body = SAMPLE_LIST,
                    headers = emptyMap(),
                    setCookies = emptyList()
                )
                path.startsWith("/moehome") -> HttpClientResponse(
                    status = 200,
                    body = SAMPLE_DETAIL,
                    headers = emptyMap(),
                    setCookies = emptyList()
                )
                path.startsWith("/moeupup") -> HttpClientResponse(
                    status = 200,
                    body = SAMPLE_IMAGES,
                    headers = emptyMap(),
                    setCookies = emptyList()
                )
                path.startsWith("/tags") -> HttpClientResponse(
                    status = 200,
                    body = SAMPLE_SEARCH,
                    headers = emptyMap(),
                    setCookies = emptyList()
                )
                else -> HttpClientResponse(200, "<html></html>", emptyMap(), emptyList())
            }
        }

        override suspend fun post(path: String, body: String, cookies: List<String>): HttpClientResponse {
            return HttpClientResponse(200, SAMPLE_SEARCH, emptyMap(), emptyList())
        }

        override suspend fun getImageBytes(url: String): ByteArray = byteArrayOf()
    }

    @Test
    fun testFetchHomepage() = runTest {
        val service = NiacgService(mockHtmlClient)
        val result = service.fetchHomepage()
        assertTrue(result.isNotEmpty())
    }

    @Test
    fun testFetchCategoryList() = runTest {
        val service = NiacgService(mockHtmlClient)
        val result = service.fetchCategoryList(3, 0)
        assertTrue(result.items.isNotEmpty())
        assertEquals("12345", result.items[0].id)
    }

    @Test
    fun testProxyThumbnail() {
        val service = NiacgService(mockHtmlClient)
        val proxyUrl = service.proxyThumbnail("https://example.com/img.jpg")
        assertTrue(proxyUrl.startsWith("/api/image?url="))
    }

    companion object {
        private val SAMPLE_HOMEPAGE = """
            <html><body>
            <h4>COS推荐</h4>
            <div class="owl-carousel">
                <div class="owl-item">
                    <div class="p-b-15">
                        <a href="moehome-1-10001.html">
                            <img data-src="https://img.example.com/cos1.jpg">
                            <span class="video-title">COS作品1</span>
                            <span class="tag">标签A</span>
                            <span class="label-category">COS</span>
                        </a>
                    </div>
                </div>
            </div>
            </body></html>
        """.trimIndent()

        private val SAMPLE_LIST = """
            <html><body>
            <div class="p-b-15">
                <a href="moehome-3-12345.html">
                    <img data-src="https://img.example.com/thumb.jpg">
                    <span class="video-title">测试本子</span>
                    <span class="tag">标签1</span>
                    <span class="label-category">本子</span>
                </a>
            </div>
            <div class="pagination">
                <a href="listinfo-3-0.html">1</a>
                <a href="listinfo-3-1.html">2</a>
                <a href="listinfo-3-9.html">尾页</a>
            </div>
            </body></html>
        """.trimIndent()

        private val SAMPLE_SEARCH = """
            <html><body>
            <div class="list-col">
                <div class="p-b-15">
                    <a href="moehome-9-67890.html">
                        <img data-src="https://img.example.com/search.jpg">
                        <span class="video-title">搜索结果</span>
                        <span class="tag">标签A</span>
                        <span class="label-category">A漫</span>
                    </a>
                </div>
            </div>
            </body></html>
        """.trimIndent()

        private val SAMPLE_DETAIL = """
            <html><body>
            <h1>详细漫画标题</h1>
            <div id="album_photo_cover">
                <img data-src="https://img.example.com/cover.jpg">
            </div>
            <div class="tag-block">
                <span data-type="author">
                    <a class="btn">作者名</a>
                </span>
                <span data-type="works">
                    <a class="btn">作品A</a>
                </span>
                <span data-type="actor">
                    <a class="btn">角色1</a>
                </span>
                <span data-type="tags">
                    <a class="btn">标签1</a>
                    <a class="btn">标签2</a>
                </span>
            </div>
            <span id="diggnum">1500</span>
            </body></html>
        """.trimIndent()

        private val SAMPLE_IMAGES = """
            <html><body>
            <img class="comic_img" data-src="https://img.boom.example.com/page1.jpg">
            <img class="comic_img" data-src="https://img.boom.example.com/page2.jpg">
            </body></html>
        """.trimIndent()
    }
}
