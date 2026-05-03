package com.niacg.backend.server

import com.niacg.backend.service.JvmTlsClient

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull()
        ?: System.getProperty("server.port")?.toIntOrNull()
        ?: 8080

    println("Starting Niacg Kotlin Backend on port $port...")
    println("Desktop mode - using JVM TLS client")
    println("API available at http://localhost:$port/api/")

    startServer(JvmTlsClient(), port)
}
