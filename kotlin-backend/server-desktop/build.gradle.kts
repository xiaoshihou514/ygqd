plugins {
    kotlin("jvm")
    kotlin("plugin.serialization")
    application
}

val copyVueDist by tasks.registering(Copy::class) {
    val vueDistDir = file("${rootProject.projectDir}/../dist")
    from(vueDistDir)
    into(layout.buildDirectory.dir("web"))
    doFirst {
        check(vueDistDir.exists()) {
            "Vue dist not found: $vueDistDir. Run 'npm run build-only' first."
        }
    }
}

tasks.named("classes") {
    dependsOn(copyVueDist)
}

application {
    mainClass.set("com.niacg.backend.server.MainKt")
}

distributions {
    main {
        contents {
            from(copyVueDist) {
                into("web")
            }
        }
    }
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    implementation(project(":core"))
    implementation("io.ktor:ktor-server-core:3.0.2")
    implementation("io.ktor:ktor-server-netty:3.0.2")
    implementation("io.ktor:ktor-server-content-negotiation:3.0.2")
    implementation("io.ktor:ktor-serialization-kotlinx-json:3.0.2")
    implementation("io.ktor:ktor-server-cors:3.0.2")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
}
