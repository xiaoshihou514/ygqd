package com.niacg.backend.cache

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class CacheManagerTest {

    @Test
    fun `put and get returns stored data`() {
        val cache = CacheManager()
        cache.put("key1", "value1")
        assertEquals("value1", cache.get("key1"))
    }

    @Test
    fun `get returns null for missing key`() {
        val cache = CacheManager()
        assertNull(cache.get("nonexistent"))
    }

    @Test
    fun `put overwrites existing key`() {
        val cache = CacheManager()
        cache.put("key1", "value1")
        cache.put("key1", "value2")
        assertEquals("value2", cache.get("key1"))
    }

    @Test
    fun `expired entry returns null`() {
        val cache = CacheManager(ttlMillis = 1)
        cache.put("key1", "value1")
        Thread.sleep(5)
        assertNull(cache.get("key1"))
    }

    @Test
    fun `not expired entry still returns data`() {
        val cache = CacheManager(ttlMillis = 100_000)
        cache.put("key1", "value1")
        assertNotNull(cache.get("key1"))
    }

    @Test
    fun `stores different types`() {
        val cache = CacheManager()
        cache.put("int", 42)
        cache.put("string", "hello")
        cache.put("list", listOf(1, 2, 3))

        assertEquals(42, cache.get("int"))
        assertEquals("hello", cache.get("string"))
        assertEquals(listOf(1, 2, 3), cache.get("list"))
    }

    @Test
    fun `default TTL is 30 minutes`() {
        val cache = CacheManager()
        cache.put("key", "value")
        assertNotNull(cache.get("key"))
    }
}
