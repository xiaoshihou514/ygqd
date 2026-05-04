package com.niacg.backend.service

import com.niacg.backend.cache.CacheManager
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class NiacgServiceTest {

    private val mockHttpClient = MockHttpClient()
    private val service = NiacgService(mockHttpClient)

    @Test
    fun `fetchHomepage returns cached data on second call`() = runTest {
        mockHttpClient.homepageBody = loadResource("homepage.html")

        val first = service.fetchHomepage()
        val second = service.fetchHomepage()

        assertEquals(1, mockHttpClient.homepageCallCount)
        assertEquals(first.size, second.size)
    }

    @Test
    fun `fetchCategoryList returns cached data on second call`() = runTest {
        mockHttpClient.listBody = loadResource("listpage.html")

        val first = service.fetchCategoryList(3, 0)
        val second = service.fetchCategoryList(3, 0)

        assertEquals(1, mockHttpClient.listCallCount)
        assertEquals(first.items.size, second.items.size)
    }

    @Test
    fun `fetchCategoryList different pages have separate cache keys`() = runTest {
        mockHttpClient.listBody = loadResource("listpage.html")

        service.fetchCategoryList(3, 0)
        service.fetchCategoryList(3, 1)

        assertEquals(2, mockHttpClient.listCallCount)
    }

    @Test
    fun `fetchCategoryList different categories have separate cache keys`() = runTest {
        mockHttpClient.listBody = loadResource("listpage.html")

        service.fetchCategoryList(3, 0)
        service.fetchCategoryList(5, 0)

        assertEquals(2, mockHttpClient.listCallCount)
    }

    @Test
    fun `searchByTags returns cached data on second call`() = runTest {
        mockHttpClient.tagSearchBody = loadResource("tagsearch.html")

        val first = service.searchByTags("testkeyword")
        val second = service.searchByTags("testkeyword")

        assertEquals(1, mockHttpClient.tagSearchCallCount)
        assertEquals(first.items.size, second.items.size)
    }

    @Test
    fun `searchByTags with fallback to list parsing`() = runTest {
        mockHttpClient.tagSearchBody = loadResource("listpage.html")

        val result = service.searchByTags("testkeyword")
        assertEquals(2, result.items.size)
    }

    @Test
    fun `searchByTags with multipage results`() = runTest {
        val page0 = loadResource("tagsearch_multipage_0.html")
        val page1 = loadResource("tagsearch_multipage_1.html")

        val client = object : HttpClient {
            var page0Calls = 0
            var page1Calls = 0

            override suspend fun get(path: String, cookies: List<String>): HttpClientResponse {
                val body = when {
                    path.contains("tags-testkeyword-0") -> {
                        page0Calls++; page0
                    }
                    path.contains("tags-testkeyword-1") -> {
                        page1Calls++; page1
                    }
                    else -> throw IllegalStateException("Unexpected path: $path")
                }
                return HttpClientResponse(200, body, emptyMap(), emptyList())
            }

            override suspend fun post(path: String, body: String, cookies: List<String>): HttpClientResponse {
                throw IllegalStateException("Not expected")
            }

            override suspend fun getImageBytes(url: String): ByteArray {
                throw IllegalStateException("Not expected")
            }
        }

        val service = NiacgService(client)
        val result = service.searchByTags("testkeyword")
        assertEquals(4, result.items.size)
        assertEquals(1, client.page0Calls)
        assertEquals(1, client.page1Calls)
    }

    @Test
    fun `searchByEngine makes post request and aggregates results`() = runTest {
        mockHttpClient.searchBody = loadResource("search.html")

        val result = service.searchByEngine(1, "test", "title,text")

        assertEquals(2, result.items.size)
        assertEquals(1, mockHttpClient.searchCallCount)
    }

    @Test
    fun `searchByEngine caches results`() = runTest {
        mockHttpClient.searchBody = loadResource("search.html")

        service.searchByEngine(1, "test", "title")
        service.searchByEngine(1, "test", "title")

        assertEquals(1, mockHttpClient.searchCallCount)
    }

    @Test
    fun `searchByEngine different params have separate cache keys`() = runTest {
        mockHttpClient.searchBody = loadResource("search.html")

        service.searchByEngine(1, "test", "title")
        service.searchByEngine(1, "other", "title")

        assertEquals(2, mockHttpClient.searchCallCount)
    }

    @Test
    fun `searchByEngine different classid has separate cache`() = runTest {
        mockHttpClient.searchBody = loadResource("search.html")

        service.searchByEngine(1, "test", "title")
        service.searchByEngine(2, "test", "title")

        assertEquals(2, mockHttpClient.searchCallCount)
    }

    @Test
    fun `searchByEngine with default show param`() = runTest {
        mockHttpClient.searchBody = loadResource("search.html")

        val result = service.searchByEngine(1, "test")
        assertEquals(2, result.items.size)
    }

    @Test
    fun `fetchComicDetail returns cached data on second call`() = runTest {
        mockHttpClient.detailBody = loadResource("detail.html")
        mockHttpClient.imagesBody = loadResource("images.html")

        val first = service.fetchComicDetail(9, "test-id")
        val second = service.fetchComicDetail(9, "test-id")

        assertEquals(1, mockHttpClient.detailCallCount)
        assertEquals(first.images.size, second.images.size)
    }

    @Test
    fun `proxyThumbnail returns proxy URL`() {
        val url = "https://example.com/image.jpg"
        val result = service.proxyThumbnail(url)
        assertTrue(result.startsWith("/api/image?url="))
        assertTrue(result.contains("example.com"))
    }

    @Test
    fun `proxyThumbnail returns empty for blank input`() {
        assertEquals("", service.proxyThumbnail(""))
        assertEquals("", service.proxyThumbnail("   "))
    }

    @Test
    fun `proxyImage returns image bytes and mime`() = runTest {
        mockHttpClient.imageBytes = byteArrayOf(1, 2, 3)

        val (bytes, mime) = service.proxyImage("https://example.com/img.jpg")
        assertEquals(3, bytes.size)
        assertEquals("image/jpeg", mime)
    }

    @Test
    fun `expired cache causes re-fetch`() = runTest {
        val noCacheService = NiacgService(mockHttpClient, CacheManager(ttlMillis = 1))
        mockHttpClient.homepageBody = loadResource("homepage.html")

        noCacheService.fetchHomepage()
        Thread.sleep(5)
        noCacheService.fetchHomepage()

        assertEquals(2, mockHttpClient.homepageCallCount)
    }

    @Test
    fun `detectMimeType handles various extensions`() = runTest {
        mockHttpClient.imageBytes = byteArrayOf()
        val tests = mapOf(
            "image.png" to "image/png",
            "image.jpg" to "image/jpeg",
            "image.jpeg" to "image/jpeg",
            "image.gif" to "image/gif",
            "image.webp" to "image/webp",
            "image.avif" to "image/avif",
            "image.png?w=100" to "image/png",
            "image.png&h=100" to "image/png",
            "image.GIF" to "image/gif",
            "image.WEBP" to "image/webp",
            "noext" to "image/jpeg",
        )
        for ((url, expectedMime) in tests) {
            val (_, mime) = service.proxyImage(url)
            assertEquals(expectedMime, mime, "Failed for $url")
        }
    }

    private fun loadResource(name: String): String {
        return NiacgServiceTest::class.java.classLoader.getResource(name)?.readText()
            ?: throw IllegalArgumentException("Resource not found: $name")
    }
}

private class MockHttpClient : HttpClient {
    var homepageBody = ""
    var listBody = ""
    var tagSearchBody = ""
    var detailBody = ""
    var imagesBody = ""
    var searchBody = ""
    var imageBytes = byteArrayOf()

    var homepageCallCount = 0
    var listCallCount = 0
    var tagSearchCallCount = 0
    var detailCallCount = 0
    var searchCallCount = 0

    override suspend fun get(path: String, cookies: List<String>): HttpClientResponse {
        return when {
            path == "/" -> {
                homepageCallCount++
                HttpClientResponse(200, homepageBody, emptyMap(), emptyList())
            }
            path.startsWith("/listinfo") -> {
                listCallCount++
                HttpClientResponse(200, listBody, emptyMap(), emptyList())
            }
            path.startsWith("/tags-testkeyword") -> {
                tagSearchCallCount++
                HttpClientResponse(200, tagSearchBody, emptyMap(), emptyList())
            }
            path.startsWith("/moehome") -> {
                detailCallCount++
                HttpClientResponse(200, detailBody, emptyMap(), emptyList())
            }
            path.startsWith("/moeupup") -> {
                HttpClientResponse(200, imagesBody, emptyMap(), emptyList())
            }
            else -> HttpClientResponse(404, "", emptyMap(), emptyList())
        }
    }

    override suspend fun post(path: String, body: String, cookies: List<String>): HttpClientResponse {
        searchCallCount++
        return HttpClientResponse(200, searchBody, emptyMap(), emptyList())
    }

    override suspend fun getImageBytes(url: String): ByteArray {
        return imageBytes
    }
}
