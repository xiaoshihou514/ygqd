plugins {
    id("com.android.application")
    kotlin("android")
    kotlin("plugin.serialization")
}

val copyVueDist = tasks.register<Copy>("copyVueDist") {
    val vueDistDir = file("${rootProject.projectDir}/../dist")
    if (vueDistDir.exists()) {
        from(vueDistDir)
        into(file("src/main/assets/web"))
    }
}

tasks.named("preBuild") {
    dependsOn(copyVueDist)
}

android {
    namespace = "com.niacg.backend.server"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.niacg.backend.server"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0.0"
    }

    buildTypes {
        debug {
            isDebuggable = true
        }
        release {
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(project(":core"))
    implementation("io.ktor:ktor-server-core:3.0.2")
    implementation("io.ktor:ktor-server-netty:3.0.2")
    implementation("io.ktor:ktor-server-content-negotiation:3.0.2")
    implementation("io.ktor:ktor-serialization-kotlinx-json:3.0.2")
    implementation("io.ktor:ktor-server-cors:3.0.2")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
}
