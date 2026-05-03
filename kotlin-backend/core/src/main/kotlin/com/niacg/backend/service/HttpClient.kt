package com.niacg.backend.service

data class HttpClientResponse(
    val status: Int,
    val body: String,
    val headers: Map<String, String>,
    val setCookies: List<String>
)

interface HttpClient {
    suspend fun get(path: String, cookies: List<String> = emptyList()): HttpClientResponse
    suspend fun post(path: String, body: String, cookies: List<String> = emptyList()): HttpClientResponse
    suspend fun getImageBytes(url: String): ByteArray
}
