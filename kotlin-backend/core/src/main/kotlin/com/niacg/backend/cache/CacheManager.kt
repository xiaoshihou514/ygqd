package com.niacg.backend.cache

import java.util.concurrent.ConcurrentHashMap

class CacheManager(private val ttlMillis: Long = 30 * 60 * 1000) {

    private data class CacheEntry(val data: Any, val expiresAt: Long)

    private val cache = ConcurrentHashMap<String, CacheEntry>()

    fun get(key: String): Any? {
        val entry = cache[key] ?: return null
        if (System.currentTimeMillis() > entry.expiresAt) {
            cache.remove(key)
            return null
        }
        return entry.data
    }

    fun put(key: String, data: Any) {
        cache[key] = CacheEntry(data, System.currentTimeMillis() + ttlMillis)
    }
}
