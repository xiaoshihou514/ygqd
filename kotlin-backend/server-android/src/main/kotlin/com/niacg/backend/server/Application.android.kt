package com.niacg.backend.server

import com.niacg.backend.db.FollowedAuthorsRepository
import com.niacg.backend.db.JdbcDatabaseProvider
import com.niacg.backend.db.TagBlacklistRepository
import com.niacg.backend.db.ViewHistoryRepository
import com.niacg.backend.service.HttpClient
import com.niacg.backend.service.NiacgService
import io.ktor.http.ContentType
import io.ktor.server.application.Application
import io.ktor.server.application.install
import io.ktor.server.plugins.contentnegotiation.ContentNegotiation
import io.ktor.server.plugins.cors.routing.CORS
import io.ktor.server.response.respondBytes
import io.ktor.server.routing.get
import io.ktor.server.routing.routing
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json
import java.io.File

fun Application.module(httpClient: HttpClient, webDir: File? = null, dbDir: File? = null) {
    install(ContentNegotiation) {
        json(Json {
            ignoreUnknownKeys = true
            prettyPrint = false
        })
    }

    install(CORS) {
        anyHost()
        allowHeaders { true }
        allowNonSimpleContentTypes = true
    }

    val dbFile = dbDir?.let { File(it, "ygqd.db") }
    val dbProvider = dbFile?.let {
        it.parentFile?.mkdirs()
        JdbcDatabaseProvider(
            jdbcUrl = "jdbc:sqlite:${it.absolutePath}",
            driverClass = "org.sqlite.JDBC",
        )
    } ?: error("dbDir must be provided on Android")
    val followsRepo = FollowedAuthorsRepository(dbProvider)
    val historyRepo = ViewHistoryRepository(dbProvider)
    val blacklistRepo = TagBlacklistRepository(dbProvider)

    routing {
        apiRoutes(NiacgService(httpClient), followsRepo, historyRepo, blacklistRepo)

        if (webDir != null && webDir.isDirectory) {
            serveWebApp(webDir)
        }
    }
}

private val MIME_TYPES = mapOf(
    "html" to "text/html",
    "css" to "text/css",
    "js" to "application/javascript",
    "mjs" to "application/javascript",
    "json" to "application/json",
    "png" to "image/png",
    "jpg" to "image/jpeg",
    "jpeg" to "image/jpeg",
    "gif" to "image/gif",
    "svg" to "image/svg+xml",
    "ico" to "image/x-icon",
    "woff" to "font/woff",
    "woff2" to "font/woff2",
    "ttf" to "font/ttf",
    "eot" to "application/vnd.ms-fontobject",
    "webp" to "image/webp",
    "avif" to "image/avif",
    "mp4" to "video/mp4",
    "webm" to "video/webm",
    "txt" to "text/plain",
    "xml" to "application/xml",
    "pdf" to "application/pdf",
)

private fun io.ktor.server.routing.Route.serveWebApp(webDir: File) {
    get("{path...}") {
        val path = call.parameters.getAll("path")?.joinToString("/") ?: ""
        val file = if (path.isEmpty()) File(webDir, "index.html") else File(webDir, path)

        if (file.exists() && file.isFile && file.canonicalPath.startsWith(webDir.canonicalPath)) {
            val ext = file.extension.lowercase()
            val mime = MIME_TYPES[ext] ?: "application/octet-stream"
            call.respondBytes(file.readBytes(), ContentType.parse(mime))
        } else {
            val indexFile = File(webDir, "index.html")
            if (indexFile.exists()) {
                call.respondBytes(indexFile.readBytes(), ContentType.Text.Html)
            }
        }
    }
}
