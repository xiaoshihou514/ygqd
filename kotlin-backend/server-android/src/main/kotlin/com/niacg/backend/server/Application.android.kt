package com.niacg.backend.server

import com.niacg.backend.service.HttpClient
import com.niacg.backend.service.NiacgService
import io.ktor.server.application.Application
import io.ktor.server.application.install
import io.ktor.server.plugins.contentnegotiation.ContentNegotiation
import io.ktor.server.plugins.cors.routing.CORS
import io.ktor.server.routing.routing
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json

fun Application.module(httpClient: HttpClient) {
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

    routing {
        apiRoutes(NiacgService(httpClient))
    }
}
