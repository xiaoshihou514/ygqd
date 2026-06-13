package com.niacg.backend.server

import com.niacg.backend.models.ApiResponse
import com.niacg.backend.models.ComicDetail
import com.niacg.backend.models.ComicItem
import com.niacg.backend.models.HomeSection
import com.niacg.backend.service.NiacgService
import com.niacg.backend.util.SplitUtil
import io.ktor.http.ContentType
import io.ktor.http.HttpStatusCode
import io.ktor.server.application.ApplicationCall
import io.ktor.server.application.call
import io.ktor.server.request.receiveStream
import io.ktor.server.response.respond
import io.ktor.server.response.respondBytes
import io.ktor.server.response.respondText
import io.ktor.server.routing.Route
import io.ktor.server.routing.get
import io.ktor.server.routing.post
import io.ktor.server.routing.route
import kotlinx.serialization.json.Json as SerializationJson

fun Route.apiRoutes(service: NiacgService) {

    route("/api") {

        get("/home") {
            handleApiCall(call) {
                val sections = service.fetchHomepage()
                rewriteThumbnailsInSections(sections, service)
            }
        }

        get("/list") {
            handleApiCall(call) {
                val cat = call.parameters["cat"]?.toIntOrNull() ?: 3
                val page = call.parameters["page"]?.toIntOrNull() ?: 0
                val result = service.fetchCategoryList(cat, page)
                result.copy(items = rewriteThumbnailsInItems(result.items, service))
            }
        }

        post("/search") {
            handleApiCall(call) {
                val body = call.receiveStream().bufferedReader().readText()
                val params = parseUrlEncoded(body)

                val keyword = params["keyword"] ?: ""
                val show = params["show"] ?: "title,text,keyboard,ftitle"
                val classid = params["classid"]?.toIntOrNull() ?: 9
                val page = params["page"]?.toIntOrNull() ?: 0
                val cacheBuster = params["_t"] ?: ""

                val result = if (show == "tags") {
                    service.searchByTags(keyword, page, cacheBuster)
                } else {
                    service.searchByEngine(
                        classid = classid, keyword = keyword, show = show,
                        page = page, cacheBuster = cacheBuster
                    )
                }

                result.copy(items = rewriteThumbnailsInItems(result.items, service))
            }
        }

        get("/comic") {
            handleApiCall(call) {
                val cat = call.parameters["cat"]?.toIntOrNull() ?: 3
                val id = call.parameters["id"]
                    ?: throw IllegalArgumentException("Missing id parameter")

                val detail = service.fetchComicDetail(cat, id)
                ComicDetail(
                    id = detail.id,
                    title = detail.title,
                    thumbnail = service.proxyThumbnail(detail.thumbnail),
                    category = detail.category,
                    categoryId = detail.categoryId,
                    author = detail.author,
                    works = detail.works,
                    characters = detail.characters,
                    tags = detail.tags,
                    likes = detail.likes,
                    images = detail.images.map { service.proxyThumbnail(it) }
                )
            }
        }

        get("/image") {
            val imageUrl = call.parameters["url"]
            if (imageUrl.isNullOrBlank()) {
                call.respondText(
                    "Missing url parameter",
                    status = HttpStatusCode.BadRequest
                )
                return@get
            }

            try {
                val (data, mime) = service.proxyImage(imageUrl)
                call.response.headers.append("Cache-Control", "public, max-age=86400")
                call.respondBytes(data, ContentType.parse(mime))
            } catch (e: Exception) {
                call.respondText(
                    "Image proxy error",
                    status = HttpStatusCode.BadGateway
                )
            }
        }

        post("/split") {
            handleApiCall(call) {
                val body = call.receiveStream().bufferedReader().readText()
                val items = SerializationJson.decodeFromString<List<ComicItem>>(body)
                SplitUtil.splitAndGroup(items)
            }
        }
    }
}

private suspend inline fun <reified T> handleApiCall(
    call: ApplicationCall,
    block: () -> T
) {
    try {
        val data = block()
        call.respond(ApiResponse(code = 0, data = data))
    } catch (e: Exception) {
        call.respond(ApiResponse<T?>(code = -1, data = null, message = e.message ?: "Internal error"))
    }
}

private fun parseUrlEncoded(body: String): Map<String, String> {
    return body.split("&").mapNotNull { part ->
        val eq = part.indexOf("=")
        if (eq == -1) null
        else java.net.URLDecoder.decode(part.substring(0, eq), "UTF-8") to
                java.net.URLDecoder.decode(part.substring(eq + 1), "UTF-8")
    }.toMap()
}

private fun rewriteThumbnailsInItems(
    items: List<ComicItem>,
    service: NiacgService
): List<ComicItem> {
    return items.map { item ->
        if (item.thumbnail.isNotBlank()) {
            item.copy(thumbnail = service.proxyThumbnail(item.thumbnail))
        } else item
    }
}

private fun rewriteThumbnailsInSections(
    sections: List<HomeSection>,
    service: NiacgService
): List<HomeSection> {
    return sections.map { section ->
        section.copy(items = rewriteThumbnailsInItems(section.items, service))
    }
}
