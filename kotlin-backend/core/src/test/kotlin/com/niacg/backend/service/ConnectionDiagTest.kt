package com.niacg.backend.service

import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertTrue

class ConnectionDiagTest {

    @Test
    fun `diagnose TLS connection and HTML response`() = runBlocking {
        val client = JvmTlsClient()

        println("=== TLS Connection Diagnostic ===")

        val response = client.get("/")
        println("Status: ${response.status}")
        println("Body length: ${response.body.length}")
        println("Set-Cookie count: ${response.setCookies.size}")
        println()

        println("=== Response Headers ===")
        response.headers.forEach { (k, v) -> println("  $k: $v") }
        println()

        println("=== Body Preview (first 2000 chars) ===")
        val preview = response.body.take(2000)
        println(preview)
        println()

        println("=== Key HTML Elements ===")
        val h4Count = Regex("<h4").findAll(response.body).count()
        val carouselCount = Regex("owl-carousel").findAll(response.body).count()
        val moehomeCount = Regex("moehome-").findAll(response.body).count()
        val bodyTag = if (response.body.contains("<body", ignoreCase = true)) "YES" else "NO"
        val htmlTag = if (response.body.contains("<html", ignoreCase = true)) "YES" else "NO"
        println("Contains <html>: $htmlTag")
        println("Contains <body>: $bodyTag")
        println("<h4> tags: $h4Count")
        println("owl-carousel: $carouselCount")
        println("moehome- links: $moehomeCount")

        assertTrue(response.status in 200..399, "Should get 2xx/3xx response")
        assertTrue(response.body.isNotEmpty(), "Should get non-empty body")
    }
}
