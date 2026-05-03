package com.niacg.backend.util

import com.niacg.backend.models.ComicItem
import com.niacg.backend.models.ParsedTitle
import com.niacg.backend.models.SplitGroup

object SplitUtil {

    private val CHINESE_NUM_MAP = mapOf(
        '一' to 1, '二' to 2, '三' to 3, '四' to 4, '五' to 5,
        '六' to 6, '七' to 7, '八' to 8, '九' to 9, '十' to 10,
        '零' to 0
    )

    private val COMPLETE_MARKERS = Regex("""(完本|全篇|END|\(完\)|完全版|全本|\b完\b)""")
    private val EXTRA_MARKERS = Regex("""(番外|特别篇)""")

    private fun chineseNumToInt(s: String): Int? {
        if (s.length == 1 && s[0] in CHINESE_NUM_MAP) return CHINESE_NUM_MAP[s[0]]
        if (s.length == 2 && s[0] == '十') {
            val unit = CHINESE_NUM_MAP[s[1]]
            return if (unit != null) 10 + unit else 10
        }
        if (s.length == 2 && s.endsWith('十')) {
            val tens = CHINESE_NUM_MAP[s[0]]
            return if (tens != null) tens * 10 else null
        }
        return null
    }

    private fun findChapterSeparator(s: String): Int {
        val patterns = listOf<Regex>(
            Regex("""第(\d+|[一二三四五六七八九十百]+)[话回章卷集]"""),
            Regex("""[话回章卷集](\d+)"""),
            Regex("""S2\s*(\d+)"""),
            Regex("""第二部\s*(\d+)"""),
            Regex("""续集\s*(\d+)"""),
            Regex("""([一二三四五六七八九十百]+)(?:\$|\s|-|,|，|、|\.)"""),
            Regex("""(\d+-\d+(?:-\d+)?)"""),
            Regex("""(?<!\w)(\d+)(?!\w|年|月|日|岁)""")
        )

        var earliest = s.length
        for (regex in patterns) {
            val result = regex.find(s)
            if (result != null && result.range.first < earliest) {
                earliest = result.range.first
            }
        }

        return if (earliest < s.length) earliest else -1
    }

    private data class ChapterParseResult(val chapters: List<Int>, val hasExtra: Boolean)

    private fun parseChapterNumbers(infoStr: String): ChapterParseResult {
        val chapters = mutableListOf<Int>()
        var hasExtra = EXTRA_MARKERS.containsMatchIn(infoStr)
        var str = EXTRA_MARKERS.replace(infoStr, "")

        Regex("""(\d+)\s*-\s*(\d+)""").findAll(str).forEach { m ->
            val a = m.groupValues[1].toInt()
            val b = m.groupValues[2].toInt()
            val start = minOf(a, b)
            val end = maxOf(a, b)
            for (i in start..end) chapters.add(i)
        }
        str = Regex("""\d+\s*-\s*\d+""").replace(str, "")

        Regex("""(\d+)""").findAll(str).forEach { m ->
            m.groupValues[1].toIntOrNull()?.let { chapters.add(it) }
        }

        Regex("""([一二三四五六七八九十百]+)""").findAll(str).forEach { m ->
            chineseNumToInt(m.groupValues[1])?.let { chapters.add(it) }
        }

        Regex("""第(\d+)[话回章卷集]""").findAll(str).forEach { m ->
            m.groupValues[1].toIntOrNull()?.let { chapters.add(it) }
        }

        Regex("""第([一二三四五六七八九十百]+)[话回章卷集]""").findAll(str).forEach { m ->
            chineseNumToInt(m.groupValues[1])?.let { chapters.add(it) }
        }

        Regex("""S2\s*(\d+)""").findAll(str).forEach { m ->
            m.groupValues[1].toIntOrNull()?.let { chapters.add(it) }
        }

        Regex("""第二部\s*(\d+)""").findAll(str).forEach { m ->
            m.groupValues[1].toIntOrNull()?.let { chapters.add(it) }
        }

        return ChapterParseResult(
            chapters = chapters.distinct().sorted(),
            hasExtra = hasExtra
        )
    }

    private fun parseTitle(item: ComicItem): ParsedTitle {
        var title = item.title
            .replace("[AI绘图]", "")
            .replace("[Chinese]", "")

        val authorMatch = Regex("""\[([^\]]+)\]""").find(title)
        val author = authorMatch?.groupValues?.get(1)?.trim() ?: ""
        var s = if (authorMatch != null) title.replace(authorMatch.value, "").trim() else title.trim()

        val isComplete = COMPLETE_MARKERS.containsMatchIn(s)
        s = COMPLETE_MARKERS.replace(s, "")
            .replace(Regex("""[()（）]"""), " ")
            .replace(Regex("""\s+"""), " ")
            .trim()

        val sepIdx = findChapterSeparator(s)

        val workName: String
        val chapterInfo: String

        if (sepIdx == -1) {
            workName = s
            chapterInfo = ""
        } else {
            workName = s.substring(0, sepIdx)
                .replace(Regex("""[-—–\s,，、]+\$"""), "")
                .trim()
            chapterInfo = s.substring(sepIdx).trim()

            if (workName.isEmpty()) {
                // fallback
                val fallbackPairs = listOf(
                    "workName" to s,
                    "chapterInfo" to ""
                )
                return ParsedTitle(
                    author = author,
                    workName = s,
                    chapters = emptyList(),
                    hasExtra = false,
                    isComplete = isComplete,
                    rawTitle = item.title
                )
            }
        }

        val (chapters, hasExtra) = if (chapterInfo.isNotEmpty()) {
            val result = parseChapterNumbers(chapterInfo)
            result.chapters to result.hasExtra
        } else {
            emptyList<Int>() to false
        }

        return ParsedTitle(
            author = author,
            workName = workName,
            chapters = chapters,
            hasExtra = hasExtra,
            isComplete = isComplete,
            rawTitle = item.title
        )
    }

    private fun groupParsed(parsed: List<ParsedTitle>): List<SplitGroup> {
        val groupMap = LinkedHashMap<String, MutableList<ParsedTitle>>()

        for (p in parsed) {
            val key = "${p.author}|${p.workName}"
            groupMap.getOrPut(key) { mutableListOf() }.add(p)
        }

        val groups = mutableListOf<SplitGroup>()

        for ((_, entries) in groupMap) {
            val first = entries.first()
            val items = entries.map { ComicItem(
                id = "", title = it.rawTitle, thumbnail = "", category = "",
                categoryId = 0, tags = emptyList(), likes = "", link = ""
            ) }

            val allChapters = mutableSetOf<Int>()
            var hasExtra = false
            var hasCompleteMark = false

            for (e in entries) {
                allChapters.addAll(e.chapters)
                if (e.hasExtra) hasExtra = true
                if (e.isComplete) hasCompleteMark = true
            }

            val knownChapters = allChapters.sorted()
            val isStandalone = knownChapters.isEmpty()

            val missingChapters = if (!isStandalone) {
                val min = 1
                val max = knownChapters.last()
                val knownSet = knownChapters.toSet()
                (min..max).filter { it !in knownSet }
            } else {
                emptyList()
            }

            val isComplete = when {
                hasCompleteMark -> true
                isStandalone -> true
                knownChapters.size == 1 -> false
                missingChapters.isNotEmpty() -> false
                else -> false
            }

            groups.add(
                SplitGroup(
                    author = first.author,
                    workName = first.workName,
                    items = items,
                    knownChapters = knownChapters,
                    missingChapters = missingChapters,
                    hasExtra = hasExtra,
                    isComplete = isComplete,
                    isStandalone = isStandalone
                )
            )
        }

        groups.sortWith(compareByDescending<SplitGroup> { it.knownChapters.size }
            .thenBy { it.author }
            .thenBy { it.workName })

        return groups
    }

    fun splitAndGroup(items: List<ComicItem>): List<SplitGroup> {
        val parsed = items.map { parseTitle(it) }
        return groupParsed(parsed)
    }
}
