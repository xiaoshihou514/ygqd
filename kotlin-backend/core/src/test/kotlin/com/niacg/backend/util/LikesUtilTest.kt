package com.niacg.backend.util

import kotlin.test.Test
import kotlin.test.assertEquals

class LikesUtilTest {

    @Test
    fun testParseLikes() {
        assertEquals(0, LikesUtil.parseLikes(""))
        assertEquals(0, LikesUtil.parseLikes("   "))
        assertEquals(123, LikesUtil.parseLikes("123"))
        assertEquals(10000, LikesUtil.parseLikes("1万"))
        assertEquals(15000, LikesUtil.parseLikes("1.5万"))
        assertEquals(5000, LikesUtil.parseLikes("5k"))
        assertEquals(3500, LikesUtil.parseLikes("3.5K"))
    }
}
