package com.niacg.backend.util

import com.niacg.backend.models.ComicItem
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class SplitUtilTest {

    private fun makeItem(id: String, title: String, tags: List<String> = emptyList()): ComicItem {
        return ComicItem(
            id = id,
            title = title,
            thumbnail = "",
            category = "CG",
            categoryId = 2,
            tags = tags,
            likes = "100",
            link = "/moehome-2-$id.html"
        )
    }

    @Test
    fun `empty list returns empty list`() {
        val result = SplitUtil.splitAndGroup(emptyList())
        assertEquals(0, result.size)
    }

    @Test
    fun `single item without chapter becomes standalone group`() {
        val item = makeItem("1", "[Author] Work Name")
        val result = SplitUtil.splitAndGroup(listOf(item))
        assertEquals(1, result.size)
        assertEquals("Author", result[0].author)
        assertEquals("Work Name", result[0].workName)
        assertTrue(result[0].isStandalone)
        assertTrue(result[0].isComplete)
    }

    @Test
    fun `items with same author and work are grouped`() {
        val items = listOf(
            makeItem("1", "[Author] Work Name 第1话"),
            makeItem("2", "[Author] Work Name 第2话"),
            makeItem("3", "[Author] Work Name 第3话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals("Author", result[0].author)
        assertEquals("Work Name", result[0].workName)
        assertFalse(result[0].isStandalone)
        assertEquals(listOf(1, 2, 3), result[0].knownChapters)
    }

    @Test
    fun `items with different authors are split`() {
        val items = listOf(
            makeItem("1", "[AuthorA] Work X 第1话"),
            makeItem("2", "[AuthorB] Work Y 第1话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(2, result.size)
    }

    @Test
    fun `items with same author but different works are split`() {
        val items = listOf(
            makeItem("1", "[Author] Work A 第1话"),
            makeItem("2", "[Author] Work B 第1话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(2, result.size)
        assertEquals("Work A", result[0].workName)
        assertEquals("Work B", result[1].workName)
    }

    @Test
    fun `missing chapters are detected`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第1话"),
            makeItem("2", "[Author] Work 第3话"),
            makeItem("3", "[Author] Work 第5话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(listOf(1, 3, 5), result[0].knownChapters)
        assertEquals(listOf(2, 4), result[0].missingChapters)
    }

    @Test
    fun `complete marker makes group complete`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第1话 完本")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertTrue(result[0].isComplete)
    }

    @Test
    fun `extra marker is detected in chapter info`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第1话 番外")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertTrue(result[0].hasExtra)
        assertFalse(result[0].isStandalone)
        assertEquals(listOf(1), result[0].knownChapters)
    }

    @Test
    fun `title without author bracket parses correctly`() {
        val items = listOf(
            makeItem("1", "Some Work 第1话"),
            makeItem("2", "Some Work 第2话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals("", result[0].author)
        assertEquals("Some Work", result[0].workName)
        assertEquals(listOf(1, 2), result[0].knownChapters)
    }

    @Test
    fun `chapter range like 1-3 is parsed`() {
        val item = makeItem("1", "[Author] Work 第1-3话")
        val result = SplitUtil.splitAndGroup(listOf(item))
        assertEquals(1, result.size)
        assertEquals(listOf(1, 2, 3), result[0].knownChapters)
    }

    @Test
    fun `groups are sorted by chapter count descending`() {
        val items = listOf(
            makeItem("1", "[A] Single"),
            makeItem("2", "[A] Multi 第1话"),
            makeItem("3", "[A] Multi 第2话"),
            makeItem("4", "[A] Multi 第3话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertTrue(result[0].knownChapters.size >= result[1].knownChapters.size)
    }

    @Test
    fun `AI tagged title is cleaned`() {
        val items = listOf(
            makeItem("1", "[Author] Work [AI绘图] 第1话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals("Work", result[0].workName)
    }

    @Test
    fun `Chinese chapter numbers are parsed`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第一话"),
            makeItem("2", "[Author] Work 第二话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(listOf(1, 2), result[0].knownChapters)
    }

    @Test
    fun `S2 format chapters are parsed`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第二部 第1话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(listOf(1), result[0].knownChapters)
    }

    @Test
    fun `result items contain raw titles`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第1话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals("[Author] Work 第1话", result[0].items[0].title)
    }

    @Test
    fun `complete marker at end is detected`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第1话 完本"),
            makeItem("2", "[Author] Work 第2话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertTrue(result[0].isComplete)
    }

    @Test
    fun `diverse title formats are parsed`() {
        val items = listOf(
            makeItem("1", "[AuthorA] Work X 1"),
            makeItem("2", "[AuthorB] Work Y - 1"),
            makeItem("3", "[AuthorC] Work Z Vol.1")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(3, result.size)
    }

    @Test
    fun `groups contain correct item count`() {
        val items = listOf(
            makeItem("1", "[A] Multi 第1话"),
            makeItem("2", "[A] Multi 第2话"),
            makeItem("3", "[A] Multi 第3话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(3, result[0].items.size)
    }

    @Test
    fun `Chinese number twelve is parsed`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第十二话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(listOf(12), result[0].knownChapters)
    }

    @Test
    fun `Chinese number twenty is parsed`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第二十话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(listOf(20), result[0].knownChapters)
    }

    @Test
    fun `Chinese number eleven is parsed`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第十一话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(listOf(11), result[0].knownChapters)
    }

    @Test
    fun `renji format is parsed`() {
        val items = listOf(
            makeItem("1", "[Author] Work 续集 1")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals(listOf(1), result[0].knownChapters)
    }

    @Test
    fun `chapter at position 0 uses fallback workName`() {
        val items = listOf(
            makeItem("1", "[Author] 1话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals("1话", result[0].workName)
        assertTrue(result[0].isStandalone)
    }

    @Test
    fun `single chapter group is not marked complete`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第1话")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertFalse(result[0].isComplete)
    }

    @Test
    fun `hasExtra set from any entry in group`() {
        val items = listOf(
            makeItem("1", "[Author] Work 第1话"),
            makeItem("2", "[Author] Work 第2话 番外")
        )
        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertTrue(result[0].hasExtra)
    }
}
