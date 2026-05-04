package com.niacg.backend.util

import kotlin.test.Test
import kotlin.test.assertEquals

class LikesUtilTest {

    @Test
    fun `parse plain number`() {
        assertEquals(1234, LikesUtil.parseLikes("1234"))
    }

    @Test
    fun `parse empty string returns 0`() {
        assertEquals(0, LikesUtil.parseLikes(""))
    }

    @Test
    fun `parse blank string returns 0`() {
        assertEquals(0, LikesUtil.parseLikes("   "))
    }

    @Test
    fun `parse wan unit`() {
        assertEquals(12000, LikesUtil.parseLikes("1.2万"))
    }

    @Test
    fun `parse wan unit without decimal`() {
        assertEquals(50000, LikesUtil.parseLikes("5万"))
    }

    @Test
    fun `parse uppercase Wan`() {
        assertEquals(30000, LikesUtil.parseLikes("3万"))
    }

    @Test
    fun `parse k unit`() {
        assertEquals(5600, LikesUtil.parseLikes("5.6k"))
    }

    @Test
    fun `parse uppercase K`() {
        assertEquals(8000, LikesUtil.parseLikes("8K"))
    }

    @Test
    fun `parse lowercase k`() {
        assertEquals(2300, LikesUtil.parseLikes("2.3k"))
    }

    @Test
    fun `parse with extra spaces`() {
        assertEquals(1000, LikesUtil.parseLikes("  1k  "))
    }

    @Test
    fun `parse invalid string returns 0`() {
        assertEquals(0, LikesUtil.parseLikes("abc"))
    }

    @Test
    fun `parse zero`() {
        assertEquals(0, LikesUtil.parseLikes("0"))
    }
}
