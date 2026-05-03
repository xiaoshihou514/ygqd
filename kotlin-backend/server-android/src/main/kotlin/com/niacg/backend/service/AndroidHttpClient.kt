package com.niacg.backend.service

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.io.IOException
import java.net.URI
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

class AndroidHttpClient(
    private val baseUrl: String = "https://www.niacg.com"
) : HttpClient {

    companion object {
        private const val UA =
            "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36"
        private const val TIMEOUT_SECONDS = 30L
    }

    private val imageClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(15, TimeUnit.SECONDS)
        .build()

    private val client = OkHttpClient.Builder()
        .connectTimeout(TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .readTimeout(TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .followRedirects(false)
        .followSslRedirects(false)
        .build()

    override suspend fun get(path: String, cookies: List<String>): HttpClientResponse {
        return executeRequest(path, cookies, null, 5)
    }

    override suspend fun post(path: String, body: String, cookies: List<String>): HttpClientResponse {
        return executeRequest(path, cookies, body, 5)
    }

    private suspend fun executeRequest(
        path: String,
        cookies: List<String>,
        requestBody: String?,
        maxRedirects: Int
    ): HttpClientResponse = withContext(Dispatchers.IO) {
        val url = if (path.startsWith("http")) path else "$baseUrl$path"

        val requestBuilder = Request.Builder()
            .url(url)
            .header("User-Agent", UA)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")

        if (cookies.isNotEmpty()) {
            requestBuilder.header("Cookie", cookies.joinToString("; "))
        }

        if (requestBody != null) {
            requestBuilder.post(
                requestBody.toRequestBody("application/x-www-form-urlencoded".toMediaType())
            )
        }

        val response = executeCall(client.newCall(requestBuilder.build()))
        val setCookies = response.headers("Set-Cookie").map { it.split(";")[0] }
        val allCookies = cookies + setCookies

        val status = response.code
        val location = response.header("Location")

        val isRedirect = status in listOf(301, 302, 303)
        if (isRedirect && location != null && maxRedirects > 0) {
            var redirectPath = location
            val urlPattern = Regex("^https?://", RegexOption.IGNORE_CASE)
            if (urlPattern.containsMatchIn(redirectPath)) {
                val u = URI.create(redirectPath)
                redirectPath = u.rawPath + if (u.rawQuery != null) "?${u.rawQuery}" else ""
            } else if (!redirectPath.startsWith("/")) {
                val base = if (path.endsWith("/")) path
                else path.substring(0, path.lastIndexOf('/') + 1)
                redirectPath = base + redirectPath
            }
            response.close()
            return@withContext executeRequest(redirectPath, allCookies, null, maxRedirects - 1)
        }

        val responseHeaders = mutableMapOf<String, String>()
        for ((name, value) in response.headers) {
            responseHeaders[name.lowercase()] = value
        }

        val bodyStr = response.body?.string() ?: ""
        response.close()

        HttpClientResponse(
            status = status,
            body = bodyStr,
            headers = responseHeaders,
            setCookies = setCookies
        )
    }

    override suspend fun getImageBytes(url: String): ByteArray = withContext(Dispatchers.IO) {
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", UA)
            .header("Referer", "https://www.niacg.com/")
            .header("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8")
            .build()

        val response = executeCall(imageClient.newCall(request))
        val bytes = response.body?.bytes() ?: byteArrayOf()
        response.close()
        bytes
    }

    private suspend fun executeCall(call: Call): Response {
        return suspendCancellableCoroutine { continuation ->
            call.enqueue(object : Callback {
                override fun onResponse(call: Call, response: Response) {
                    continuation.resume(response)
                }

                override fun onFailure(call: Call, e: IOException) {
                    continuation.resumeWithException(e)
                }
            })
            continuation.invokeOnCancellation { call.cancel() }
        }
    }
}
