package com.niacg.backend.util

object LikesUtil {
    fun parseLikes(likes: String): Int {
        val cleaned = likes.trim()
        if (cleaned.isEmpty()) return 0

        val numStr = cleaned.replace(Regex("[^0-9.]"), "")
        val num = numStr.toDoubleOrNull() ?: return 0

        return when {
            Regex("万", RegexOption.IGNORE_CASE).containsMatchIn(cleaned) -> (num * 10000).toInt()
            Regex("k", RegexOption.IGNORE_CASE).containsMatchIn(cleaned) -> (num * 1000).toInt()
            else -> num.toInt()
        }
    }
}
