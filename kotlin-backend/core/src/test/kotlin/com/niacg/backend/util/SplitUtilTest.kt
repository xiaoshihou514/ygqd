package com.niacg.backend.util

import com.niacg.backend.models.ComicItem
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class SplitUtilTest {

    @Test
    fun testSplitSingleItem() {
        val items = listOf(
            ComicItem(
                id = "1", title = "[作者A] 作品名 1-8(完)",
                thumbnail = "", category = "", categoryId = 0,
                tags = emptyList(), likes = "", link = ""
            )
        )

        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals("作者A", result[0].author)
        assertEquals("作品名", result[0].workName)
        assertTrue(result[0].isComplete)
    }

    @Test
    fun testSplitMultipleItemsSameWork() {
        val items = listOf(
            ComicItem(
                id = "1", title = "[作者B] 作品X 1-5",
                thumbnail = "", category = "", categoryId = 0,
                tags = emptyList(), likes = "", link = ""
            ),
            ComicItem(
                id = "2", title = "[作者B] 作品X 6-10",
                thumbnail = "", category = "", categoryId = 0,
                tags = emptyList(), likes = "", link = ""
            )
        )

        val result = SplitUtil.splitAndGroup(items)
        assertEquals(1, result.size)
        assertEquals("作者B", result[0].author)
        assertEquals("作品X", result[0].workName)
        assertEquals(10, result[0].knownChapters.size)
    }

    @Test
    fun testSplitEmpty() {
        val result = SplitUtil.splitAndGroup(emptyList())
        assertEquals(0, result.size)
    }
}
