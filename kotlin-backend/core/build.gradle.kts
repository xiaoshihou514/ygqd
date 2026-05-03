plugins {
    kotlin("jvm")
    kotlin("plugin.serialization")
}

group = "com.niacg.backend"
version = "1.0.0"

kotlin {
    jvmToolchain(17)
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")
    implementation("org.jsoup:jsoup:1.18.1")

    testImplementation(kotlin("test"))
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0")
}

tasks.test {
    useJUnitPlatform {
        if (project.hasProperty("excludeIntegration")) {
            excludeTags("integration")
        }
    }
}
