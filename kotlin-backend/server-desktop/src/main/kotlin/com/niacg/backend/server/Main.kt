package com.niacg.backend.server

import com.niacg.backend.service.HttpClient
import com.niacg.backend.service.JvmTlsClient
import java.io.File

fun main() {
    val webDir = locateWebDir()
    val httpClient: HttpClient = JvmTlsClient()

    println("Server starting on http://localhost:8080")
    if (webDir != null) {
        println("Serving Vue frontend from: ${webDir.absolutePath}")
    }

    startServer(httpClient, webDir)
}

private fun locateWebDir(): File? {
    val candidates = listOfNotNull(
        System.getProperty("niacg.webDir")?.let { File(it) },
        findRelativeToJar(),
        findRelativeToCwd(),
    )
    return candidates.firstOrNull { it.isDirectory }
}

private fun findRelativeToJar(): File? {
    return try {
        val jarUrl = JvmTlsClient::class.java.protectionDomain.codeSource.location
        if (jarUrl?.protocol == "file") {
            File(jarUrl.toURI()).parentFile?.let { parent ->
                File(parent, "web")
            }
        } else null
    } catch (_: Exception) {
        null
    }
}

private fun findRelativeToCwd(): File? {
    val cwd = File(System.getProperty("user.dir"))
    val webDir = File(cwd, "web")
    if (webDir.isDirectory) return webDir
    val kotlinWebDir = File(cwd, "kotlin-backend/server-desktop/build/web")
    if (kotlinWebDir.isDirectory) return kotlinWebDir
    return null
}
