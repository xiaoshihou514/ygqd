package com.niacg.backend.parser

import com.niacg.backend.models.CategoryListResult
import com.niacg.backend.models.ComicDetail
import com.niacg.backend.models.ComicItem
import com.niacg.backend.models.HomeSection
import com.niacg.backend.models.PaginationInfo
import com.niacg.backend.models.SearchResult
import org.jsoup.Jsoup
import org.jsoup.nodes.Document
import org.jsoup.nodes.Element

object HtmlParser {

    private const val NIACG_BASE = "https://www.niacg.com"

    private val CATEGORY_MAP = mapOf(
        "1" to "COS",
        "2" to "CG",
        "3" to "本子",
        "4" to "套图",
        "9" to "A漫"
    )

    fun resolveUrl(url: String): String {
        if (url.isBlank()) return ""
        if (url.startsWith("http://") || url.startsWith("https://") || url.startsWith("//")) return url
        if (url.startsWith("/")) return NIACG_BASE + url
        return "$NIACG_BASE/$url"
    }

    private data class SectionMatch(
        val heading: String,
        val categoryId: Int,
        val category: String,
        val label: String
    )

    fun parseHomepage(html: String): List<HomeSection> {
        val doc = Jsoup.parse(html)
        val sections = mutableListOf<HomeSection>()

        val elements = doc.body().select("h4, .owl-carousel")
        var lastHeading: Element? = null

        for (el in elements) {
            when {
                el.normalName() == "h4" -> lastHeading = el
                el.hasClass("owl-carousel") && lastHeading != null -> {
                    val heading = lastHeading!!.text().trim()
                    val match = matchHeading(heading)
                    if (match != null) {
                        val items = mutableListOf<ComicItem>()
                        el.select(".owl-item .p-b-15").forEach { wrapper ->
                            extractItem(wrapper)?.let { items.add(it) }
                        }
                        if (items.isNotEmpty()) {
                            sections.add(
                                HomeSection(
                                    category = match.category,
                                    categoryId = match.categoryId,
                                    label = match.label,
                                    items = items
                                )
                            )
                        }
                    }
                    lastHeading = null
                }
            }
        }

        return sections
    }

    private fun matchHeading(heading: String): SectionMatch? {
        return when {
            heading.contains("COS") -> SectionMatch(heading, 1, "COS", "COS推荐")
            heading.contains("套图") -> SectionMatch(heading, 4, "套图", "套图推荐")
            heading.contains("CG") -> SectionMatch(heading, 2, "CG", "CG推荐")
            heading.contains("本本") || heading.contains("本子") -> SectionMatch(heading, 3, "本子", "本子推荐")
            heading.contains("里番") -> SectionMatch(heading, 19, "里番", "里番推荐")
            heading.contains("3D") -> SectionMatch(heading, 20, "3D", "3D推荐")
            heading.contains("同人") -> SectionMatch(heading, 21, "同人", "同人推荐")
            else -> null
        }
    }

    fun parseListPage(html: String): CategoryListResult {
        val doc = Jsoup.parse(html)
        val items = mutableListOf<ComicItem>()
        val seen = mutableSetOf<String>()

        val selectors = listOf(
            ".owl-item .p-b-15",
            ".owl-item",
            ".list-col .p-b-15",
            ".p-b-15"
        )

        for (selector in selectors) {
            for (el in doc.select(selector)) {
                extractItem(el)?.let { item ->
                    if (seen.add(item.id)) {
                        items.add(item)
                    }
                }
            }
        }

        val pagination = extractPagination(doc)
        return CategoryListResult(items = items, pagination = pagination)
    }

    private fun extractPagination(doc: Document): PaginationInfo {
        val pageLinks = doc.select(".pagination a")
        val urls = pageLinks.mapNotNull { it.attr("href").takeIf { h -> h.contains("listinfo") } }

        val pageMatch = Regex("""listinfo-\d+-(\d+)\.html""")
            .find(urls.firstOrNull() ?: "")?.groupValues?.get(1)?.toIntOrNull() ?: 0

        val total = urls.lastOrNull()?.let {
            Regex("""listinfo-\d+-(\d+)\.html""").find(it)?.groupValues?.get(1)?.toIntOrNull()
        } ?: (pageMatch + 1)

        return PaginationInfo(
            current = pageMatch,
            total = total,
            hasNext = pageMatch < total,
            hasPrev = pageMatch > 0
        )
    }

    fun parseSearchResults(html: String): SearchResult {
        val doc = Jsoup.parse(html)
        val items = mutableListOf<ComicItem>()
        val seen = mutableSetOf<String>()

        val selectors = listOf(
            ".list-col .p-b-15",
            ".owl-item .p-b-15",
            ".p-b-15"
        )

        for (selector in selectors) {
            for (el in doc.select(selector)) {
                extractItem(el)?.let { item ->
                    if (seen.add(item.id)) {
                        items.add(item)
                    }
                }
            }
        }

        val (pagination, pageUrlTemplate) = extractSearchPagination(doc)
        return SearchResult(items = items, pagination = pagination, pageUrlTemplate = pageUrlTemplate)
    }

    fun extractTagSearchMaxPage(html: String, keyword: String): Int {
        val doc = Jsoup.parse(html)
        val pageLinks = doc.select(".pagination a")
        val urls = pageLinks.map { it.attr("href") }.filter { it.isNotBlank() }

        val escapedKeyword = Regex.escape(keyword)
        val regex = Regex("""tags-$escapedKeyword-(\d+)\.html""")

        return urls.mapNotNull { regex.find(it)?.groupValues?.get(1)?.toIntOrNull() }
            .maxOrNull() ?: 0
    }

    private fun extractSearchPagination(doc: Document): Pair<PaginationInfo, String?> {
        val pageLinks = doc.select(".pagination a")
        val urls = pageLinks.mapNotNull { it.attr("href").takeIf { h -> h.isNotBlank() } }

        val pageNums = urls.mapNotNull {
            Regex("""[?&]page=(\d+)""", RegexOption.IGNORE_CASE)
                .find(it)?.groupValues?.get(1)?.toIntOrNull()
        }

        val total = if (pageNums.isNotEmpty()) pageNums.max() else 1
        val current = 1

        val pageUrlTemplate = urls.firstOrNull()?.replace(
            Regex("""page=\d+""", RegexOption.IGNORE_CASE), "page={}"
        )

        return Pair(
            PaginationInfo(
                current = current - 1,
                total = total - 1,
                hasNext = current < total,
                hasPrev = current > 1
            ),
            pageUrlTemplate
        )
    }

    fun parseComicDetail(html: String, categoryId: Int, id: String): ComicDetail {
        val doc = Jsoup.parse(html)

        val title = (doc.selectFirst("h1")
            ?: doc.selectFirst(".panel-title"))?.text()?.trim() ?: ""

        val coverImg = doc.selectFirst("#album_photo_cover img")
        val rawThumbnail = coverImg?.let {
            it.attr("data-src")
                .ifEmpty { it.attr("data-original") }
                .ifEmpty { it.attr("src") }
        } ?: ""
        val thumbnail = resolveUrl(rawThumbnail)

        val authorSpan = doc.selectFirst(".tag-block [data-type=\"author\"]")
        val authorEls = authorSpan?.select("a.btn") ?: emptyList<Element>().toList()
        val authorName = authorEls.map { it.text().trim() }.filter { it.isNotEmpty() }.joinToString(", ")

        val workSpan = doc.selectFirst(".tag-block [data-type=\"works\"]")
        val workEls = workSpan?.select("a.btn") ?: emptyList<Element>().toList()
        val works = workEls.mapNotNull { it.text().trim().takeIf { t -> t.isNotEmpty() } }

        val charSpan = doc.selectFirst(".tag-block [data-type=\"actor\"]")
        val charEls = charSpan?.select("a.btn") ?: emptyList<Element>().toList()
        val characters = charEls.mapNotNull { it.text().trim().takeIf { t -> t.isNotEmpty() } }

        val tagSpan = doc.selectFirst(".tag-block [data-type=\"tags\"]")
        val tagBtnEls = tagSpan?.select("a.btn") ?: emptyList<Element>().toList()
        val tags = tagBtnEls.mapNotNull { it.text().trim().takeIf { t -> t.isNotEmpty() } }

        val likesEl = doc.selectFirst("#diggnum") ?: doc.selectFirst("[id^=\"albim_likes_\"]")
        val likes = likesEl?.text()?.trim() ?: ""

        val catEl = doc.selectFirst(".label-category, .label-sub")
        val category = catEl?.text()?.trim() ?: CATEGORY_MAP[categoryId.toString()] ?: ""

        return ComicDetail(
            id = id,
            title = decodeHtmlEntities(title),
            thumbnail = thumbnail,
            category = category,
            categoryId = categoryId,
            author = authorName,
            works = works,
            characters = characters,
            tags = tags,
            likes = likes,
            images = emptyList()
        )
    }

    fun parseComicImages(html: String): List<String> {
        val doc = Jsoup.parse(html)
        val imgEls = doc.select("img.comic_img")
        val images = mutableListOf<String>()
        val seen = mutableSetOf<String>()

        for (el in imgEls) {
            val url = listOf(
                el.attr("data-src"),
                el.attr("data-original"),
                el.attr("src")
            ).firstOrNull { it.isNotBlank() } ?: ""

            if (url.isNotBlank() && seen.add(url) &&
                (url.contains("boom") || url.contains("xunge") || url.contains("hen"))
            ) {
                images.add(resolveUrl(url))
            }
        }

        return images
    }

    fun parseListAsSearchResult(html: String): SearchResult {
        val listResult = parseListPage(html)
        return SearchResult(
            items = listResult.items,
            pagination = PaginationInfo(0, 0, false, false)
        )
    }

    private fun extractItem(el: Element): ComicItem? {
        val linkEl = el.selectFirst("a[href*=\"moehome\"]") ?: return null
        val href = linkEl.attr("href")

        val match = Regex("""moehome-(\d+)-(\d+)\.html""").find(href) ?: return null
        val categoryId = match.groupValues[1].toInt()
        val id = match.groupValues[2]

        val imgEl = el.selectFirst("img")
        val rawThumbnail = imgEl?.let {
            it.attr("data-src")
                .ifEmpty { it.attr("data-original") }
                .ifEmpty { it.attr("src") }
        } ?: ""
        val thumbnail = resolveUrl(rawThumbnail)

        val titleEl = el.selectFirst(".video-title")
            ?: el.parent()?.selectFirst(".video-title")
        val title = titleEl?.text()?.trim() ?: ""

        val tagEls = el.select(".tag")
        val parentTags = el.parent()?.select(".tag") ?: emptyList<Element>().toList()
        val allTags = if (tagEls.isNotEmpty()) tagEls else parentTags
        val tags = allTags.map { it.text().trim() }.filter { it.isNotEmpty() }

        val likeEl = el.selectFirst("[id^=\"albim_likes_\"]")
        val likes = likeEl?.text()?.trim() ?: ""

        val catEl = el.selectFirst(".label-category, .label-sub")
        val category = catEl?.text()?.trim()
            ?: CATEGORY_MAP[categoryId.toString()] ?: ""

        return ComicItem(
            id = id,
            title = decodeHtmlEntities(title),
            thumbnail = thumbnail,
            category = category,
            categoryId = categoryId,
            tags = tags,
            likes = likes,
            link = href
        )
    }

    private fun decodeHtmlEntities(text: String): String {
        return text
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
    }
}
