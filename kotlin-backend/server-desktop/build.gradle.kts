plugins {
    kotlin("jvm")
    kotlin("plugin.serialization")
    application
}

group = "com.niacg.backend"
version = "1.0.0"

application {
    mainClass.set("com.niacg.backend.server.MainKt")
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    implementation(project(":core"))
    implementation("io.ktor:ktor-server-core:3.0.2")
    implementation("io.ktor:ktor-server-cio:3.0.2")
    implementation("io.ktor:ktor-server-content-negotiation:3.0.2")
    implementation("io.ktor:ktor-serialization-kotlinx-json:3.0.2")
    implementation("io.ktor:ktor-server-cors:3.0.2")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")
    implementation("ch.qos.logback:logback-classic:1.5.12")

    testImplementation(kotlin("test"))
    testImplementation("io.ktor:ktor-server-test-host:3.0.2")
}

tasks.test {
    useJUnitPlatform()
}
