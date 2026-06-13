package com.niacg.backend.service

import com.niacg.backend.cache.CacheManager
import com.niacg.backend.models.CategoryListResult
import com.niacg.backend.models.ComicDetail
import com.niacg.backend.models.HomeSection
import com.niacg.backend.models.PaginationInfo
import com.niacg.backend.models.SearchResult
import com.niacg.backend.parser.HtmlParser

class NiacgService(
    private val httpClient: HttpClient,
    private val cache: CacheManager = CacheManager()
) {

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

    suspend fun searchByTags(
        keyword: String,
        page: Int = 0,
        cacheBuster: String = ""
    ): SearchResult {
        val cacheKey = buildString {
            append("search-tags-$keyword-$page")
            if (cacheBuster.isNotBlank()) append("-$cacheBuster")
        }
        val cached = cache.get(cacheKey)
        if (cached != null) {
            @Suppress("UNCHECKED_CAST")
            return cached as SearchResult
        }
        val encodedKeyword = java.net.URLEncoder.encode(keyword, "UTF-8")
        val response = httpClient.get("/tags-$encodedKeyword-$page.html")
        var result = HtmlParser.parseSearchResults(response.body)
        if (result.items.isEmpty()) {
            result = HtmlParser.parseListAsSearchResult(response.body)
        }
        val maxPage = HtmlParser.extractTagSearchMaxPage(response.body, keyword)
        val totalPages = maxPage + 1
        val data = SearchResult(
            items = result.items,
            pagination = PaginationInfo(
                current = page,
                total = totalPages,
                hasNext = page + 1 < totalPages,
                hasPrev = page > 0
            )
        )
        cache.put(cacheKey, data)
        return data
    }

    suspend fun searchByEngine(
        classid: Int,
        keyword: String,
        show: String = "title,text,keyboard,ftitle",
        page: Int = 0,
        cacheBuster: String = ""
    ): SearchResult {
        val cacheKey = buildString {
            append("search-$classid-$keyword-$show-$page")
            if (cacheBuster.isNotBlank()) append("-$cacheBuster")
        }
        val cached = cache.get(cacheKey)
        if (cached != null) {
            @Suppress("UNCHECKED_CAST")
            return cached as SearchResult
        }
        val searchBody = buildString {
            append("classid=$classid")
            append("&keyboard=${java.net.URLEncoder.encode(keyword, "UTF-8")}")
            append("&show=${java.net.URLEncoder.encode(show, "UTF-8")}")
            append("&tempid=1")
            append("&Submit=")
        }

        val firstResponse = httpClient.post("/e/search/index.php", searchBody)
        val firstResult = HtmlParser.parseSearchResults(firstResponse.body)

        val data = if (page == 0) {
            val pageCount = firstResult.pagination.total + 1
            SearchResult(
                items = firstResult.items,
                pagination = PaginationInfo(
                    current = 0,
                    total = pageCount,
                    hasNext = pageCount > 1,
                    hasPrev = false
                ),
                pageUrlTemplate = firstResult.pageUrlTemplate
            )
        } else {
            val pageUrlTemplate = firstResult.pageUrlTemplate
            if (pageUrlTemplate.isNullOrBlank()) {
                throw IllegalStateException("No pagination template available for page $page")
            }
            val cookies = firstResponse.setCookies
            val pageUrl = pageUrlTemplate.replace("{}", (page + 1).toString())
            val pageResponse = httpClient.get(pageUrl, cookies)
            val pageResult = HtmlParser.parseSearchResults(pageResponse.body)
            val totalPages = firstResult.pagination.total + 1
            SearchResult(
                items = pageResult.items,
                pagination = PaginationInfo(
                    current = page,
                    total = totalPages,
                    hasNext = page + 1 < totalPages,
                    hasPrev = page > 0
                )
            )
        }
        cache.put(cacheKey, data)
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
