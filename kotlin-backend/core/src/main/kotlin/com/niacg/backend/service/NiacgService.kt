package com.niacg.backend.service

import com.niacg.backend.cache.CacheManager
import com.niacg.backend.models.CategoryListResult
import com.niacg.backend.models.ComicDetail
import com.niacg.backend.models.ComicItem
import com.niacg.backend.models.HomeSection
import com.niacg.backend.models.PaginationInfo
import com.niacg.backend.models.SearchResult
import com.niacg.backend.parser.HtmlParser
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope

class NiacgService(
    private val httpClient: HttpClient,
    private val cache: CacheManager = CacheManager()
) {

    companion object {
        private const val CONCURRENCY = 32
    }

    suspend fun fetchHomepage(): List<HomeSection> {
        val key = "homepage"
        val cached = cache.get(key)
        if (cached != null) {
            @Suppress("UNCHECKED_CAST")
            return cached as List<HomeSection>
        }
        val response = httpClient.get("/")
        val data = HtmlParser.parseHomepage(response.body)
        cache.put(key, data)
        return data
    }

    suspend fun fetchCategoryList(category: Int, page: Int): CategoryListResult {
        val key = "list-$category-$page"
        val cached = cache.get(key)
        if (cached != null) {
            @Suppress("UNCHECKED_CAST")
            return cached as CategoryListResult
        }
        val response = httpClient.get("/listinfo-$category-$page.html")
        val data = HtmlParser.parseListPage(response.body)
        cache.put(key, data)
        return data
    }

    suspend fun searchByTags(keyword: String): SearchResult {
        val key = "search-tags-$keyword"
        val cached = cache.get(key)
        if (cached != null) {
            @Suppress("UNCHECKED_CAST")
            return cached as SearchResult
        }
        val data = coroutineScope {
            val encodedKeyword = java.net.URLEncoder.encode(keyword, "UTF-8")
            val firstResponse = httpClient.get("/tags-$encodedKeyword-0.html")

            var firstResult = HtmlParser.parseSearchResults(firstResponse.body)
            if (firstResult.items.isEmpty()) {
                firstResult = HtmlParser.parseListAsSearchResult(firstResponse.body)
            }

            val allItems = mutableListOf<ComicItem>()
            allItems.addAll(firstResult.items)

            val maxPage = HtmlParser.extractTagSearchMaxPage(firstResponse.body, keyword)
            if (maxPage > 0) {
                var i = 1
                while (i <= maxPage) {
                    val batch = (i until minOf(i + CONCURRENCY, maxPage + 1)).map { j ->
                        async {
                            runCatching {
                                val resp = httpClient.get("/tags-$encodedKeyword-$j.html")
                                var result = HtmlParser.parseSearchResults(resp.body)
                                if (result.items.isEmpty()) {
                                    result = HtmlParser.parseListAsSearchResult(resp.body)
                                }
                                result.items
                            }.getOrElse { emptyList() }
                        }
                    }
                    batch.awaitAll().forEach { allItems.addAll(it) }
                    i += CONCURRENCY
                }
            }

            SearchResult(
                items = allItems,
                pagination = PaginationInfo(0, 0, false, false)
            )
        }
        cache.put(key, data)
        return data
    }

    suspend fun searchByEngine(
        classid: Int,
        keyword: String,
        show: String = "title,text,keyboard,ftitle"
    ): SearchResult {
        val key = "search-$classid-$keyword-$show"
        val cached = cache.get(key)
        if (cached != null) {
            @Suppress("UNCHECKED_CAST")
            return cached as SearchResult
        }
        val data = coroutineScope {
            val searchBody = buildString {
                append("classid=$classid")
                append("&keyboard=${java.net.URLEncoder.encode(keyword, "UTF-8")}")
                append("&show=${java.net.URLEncoder.encode(show, "UTF-8")}")
                append("&tempid=1")
                append("&Submit=")
            }

            val firstResponse = httpClient.post("/e/search/index.php", searchBody)
            val firstResult = HtmlParser.parseSearchResults(firstResponse.body)
            val allItems = mutableListOf<ComicItem>()
            allItems.addAll(firstResult.items)
            val cookies = firstResponse.setCookies

            if (firstResult.pageUrlTemplate != null && firstResult.pagination.total > 0) {
                val pageCount = firstResult.pagination.total + 1
                var i = 2
                while (i <= pageCount) {
                    val batch = (i until minOf(i + CONCURRENCY, pageCount + 1)).map { j ->
                        async {
                            runCatching {
                                val pageUrl = firstResult.pageUrlTemplate!!.replace("{}", j.toString())
                                val resp = httpClient.get(pageUrl, cookies)
                                HtmlParser.parseSearchResults(resp.body).items
                            }.getOrElse { emptyList() }
                        }
                    }
                    batch.awaitAll().forEach { allItems.addAll(it) }
                    i += CONCURRENCY
                }
            }

            SearchResult(
                items = allItems,
                pagination = PaginationInfo(0, 0, false, false)
            )
        }
        cache.put(key, data)
        return data
    }

    suspend fun fetchComicDetail(categoryId: Int, id: String): ComicDetail {
        val key = "detail-$categoryId-$id"
        val cached = cache.get(key)
        if (cached != null) {
            @Suppress("UNCHECKED_CAST")
            return cached as ComicDetail
        }
        val detailResponse = httpClient.get("/moehome-$categoryId-$id.html")
        val detail = HtmlParser.parseComicDetail(detailResponse.body, categoryId, id)

        val imagesResponse = httpClient.get("/moeupup-$categoryId-$id.html")
        val images = HtmlParser.parseComicImages(imagesResponse.body)

        val data = detail.copy(images = images)
        cache.put(key, data)
        return data
    }

    suspend fun proxyImage(imageUrl: String): Pair<ByteArray, String> {
        val data = httpClient.getImageBytes(imageUrl)
        val mime = detectMimeType(imageUrl)
        return Pair(data, mime)
    }

    private fun detectMimeType(url: String): String {
        val lower = url.lowercase()
        return when {
            lower.endsWith(".png") || lower.contains(".png?") || lower.contains(".png&") -> "image/png"
            lower.endsWith(".gif") || lower.contains(".gif?") || lower.contains(".gif&") -> "image/gif"
            lower.endsWith(".webp") || lower.contains(".webp?") || lower.contains(".webp&") -> "image/webp"
            lower.endsWith(".avif") || lower.contains(".avif?") || lower.contains(".avif&") -> "image/avif"
            else -> "image/jpeg"
        }
    }

    fun proxyThumbnail(url: String): String {
        if (url.isBlank()) return ""
        return "/api/image?url=${java.net.URLEncoder.encode(url, "UTF-8")}"
    }
}
