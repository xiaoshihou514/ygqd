package com.niacg.backend.service

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.nio.charset.StandardCharsets
import java.nio.file.Path

class JvmTlsClient : HttpClient {

    companion object {
        private const val UA =
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36"
    }

    private fun curlHeaders(cookies: List<String>): List<String> {
        val headers = mutableListOf(
            "-H", "User-Agent: $UA",
            "-H", "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "-H", "Accept-Language: zh-CN,zh;q=0.9,en;q=0.8",
            "--compressed",
            "-s", "-i", "--max-time", "30",
        )
        if (cookies.isNotEmpty()) {
            headers.addAll(listOf("-H", "Cookie: ${cookies.joinToString("; ")}"))
        }
        return headers
    }

    private suspend fun curlRequest(
        url: String,
        method: String = "GET",
        body: String? = null,
        cookies: List<String> = emptyList(),
        maxRedirects: Int = 5,
        outputFile: Path? = null
    ): HttpClientResponse = withContext(Dispatchers.IO) {
        val args = mutableListOf("curl", "-L", "--max-redirs", maxRedirects.toString())
        args.addAll(curlHeaders(cookies))
        args.add("-X")
        args.add(method)
        if (body != null) {
            args.addAll(listOf("-H", "Content-Type: application/x-www-form-urlencoded"))
            args.addAll(listOf("--data-raw", body))
        }
        args.add(url)

        val proc = ProcessBuilder(args)
            .redirectErrorStream(true)
            .start()

        val allBytes = proc.inputStream.readBytes()
        proc.waitFor()

        val rawOutput = String(allBytes, StandardCharsets.UTF_8)
        val crlfIdx = rawOutput.indexOf("\r\n\r\n")
        val headerPart = if (crlfIdx > 0) rawOutput.substring(0, crlfIdx) else rawOutput
        val bodyPart = if (crlfIdx > 0) rawOutput.substring(crlfIdx + 4) else ""

        val headerLines = headerPart.split("\r\n")
        val statusLine = headerLines.firstOrNull() ?: "HTTP/1.1 200"
        val status = statusLine.split(" ").getOrNull(1)?.toIntOrNull() ?: 200

        val responseHeaders = mutableMapOf<String, String>()
        val setCookies = mutableListOf<String>()

        for (line in headerLines.drop(1)) {
            val colonIdx = line.indexOf(": ")
            if (colonIdx == -1) continue
            val key = line.substring(0, colonIdx).lowercase()
            val value = line.substring(colonIdx + 2)

            if (key == "set-cookie") {
                setCookies.add(value.split(";")[0])
            } else {
                responseHeaders[key] = value
            }
        }

        HttpClientResponse(
            status = status,
            body = bodyPart,
            headers = responseHeaders,
            setCookies = setCookies
        )
    }

    override suspend fun get(path: String, cookies: List<String>): HttpClientResponse {
        val url = if (path.startsWith("http")) path else "https://www.niacg.com$path"
        return curlRequest(url, "GET", null, cookies)
    }

    override suspend fun post(path: String, body: String, cookies: List<String>): HttpClientResponse {
        val url = if (path.startsWith("http")) path else "https://www.niacg.com$path"
        return curlRequest(url, "POST", body, cookies)
    }

    override suspend fun getImageBytes(url: String): ByteArray = withContext(Dispatchers.IO) {
        val proc = ProcessBuilder(
            "curl", "-s", "--max-time", "15", "-L",
            "-H", "User-Agent: $UA",
            "-H", "Referer: https://www.niacg.com/",
            "-H", "Accept: image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
            url
        ).redirectErrorStream(true).start()

        proc.inputStream.readBytes()
    }
}
