plugins {
    id("com.android.application")
    kotlin("android")
    kotlin("plugin.serialization")
}

val copyVueDist = tasks.register<Copy>("copyVueDist") {
    val vueDistDir = file("${rootProject.projectDir}/../dist")
    from(vueDistDir)
    into(file("src/main/assets/web"))
    doFirst {
        check(vueDistDir.exists()) {
            "Vue dist directory not found: $vueDistDir. Run 'npm run build-only' first."
        }
    }
}

val sqliteVersion = "3.53.2.0"

val extractSqliteNatives = tasks.register<Copy>("extractSqliteNatives") {
    val nativesJar = configurations.detachedConfiguration(
        dependencies.create("org.xerial:sqlite-jdbc:$sqliteVersion:natives-android")
    ).singleFile
    from(zipTree(nativesJar)) {
        include("**/*.so")
    }
    eachFile {
        val parent = relativePath.parent?.pathString ?: ""
        val dirName = parent.substringAfterLast("/")
        val abiMap = mapOf(
            "aarch64" to "arm64-v8a",
            "arm" to "armeabi",
            "x86" to "x86",
            "x86_64" to "x86_64",
        )
        path = "${abiMap[dirName] ?: dirName}/${name}"
    }
    into(file("src/main/jniLibs"))
    includeEmptyDirs = false
}

tasks.named("preBuild") {
    dependsOn(copyVueDist, extractSqliteNatives)
}

android {
    namespace = "com.niacg.backend.server"
    compileSdk = 35

    val keystoreFile = rootProject.file("../release.keystore")
    val storePass = System.getenv("KEYSTORE_PASSWORD") ?: "niacg123456"
    val aliasName = System.getenv("KEY_ALIAS") ?: "niacg"
    val keyPass = System.getenv("KEY_PASSWORD") ?: "niacg123456"

    signingConfigs {
        create("release") {
            if (keystoreFile.exists()) {
                storeFile = keystoreFile
                storePassword = storePass
                keyAlias = aliasName
                keyPassword = keyPass
            }
        }
    }

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
            isMinifyEnabled = false
            signingConfig = if (keystoreFile.exists()) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }

    packaging {
        resources {
            excludes += "META-INF/INDEX.LIST"
            excludes += "META-INF/io.netty.versions.properties"
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
    implementation("org.jetbrains.exposed:exposed-core:0.51.1")
    implementation("org.jetbrains.exposed:exposed-jdbc:0.51.1")
    implementation("org.xerial:sqlite-jdbc:$sqliteVersion")
    implementation("io.ktor:ktor-server-core:3.0.2")
    implementation("io.ktor:ktor-server-netty:3.0.2")
    implementation("io.ktor:ktor-server-content-negotiation:3.0.2")
    implementation("io.ktor:ktor-serialization-kotlinx-json:3.0.2")
    implementation("io.ktor:ktor-server-cors:3.0.2")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
}
