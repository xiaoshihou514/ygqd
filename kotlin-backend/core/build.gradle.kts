plugins {
    kotlin("jvm")
    kotlin("plugin.serialization")
    jacoco
}

group = "com.niacg.backend"
version = "1.0.0"

kotlin {
    jvmToolchain(17)
    compilerOptions {
        allWarningsAsErrors.set(true)
    }
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
    finalizedBy(tasks.jacocoTestReport)
}

tasks.jacocoTestReport {
    dependsOn(tasks.test)
    reports {
        xml.required.set(true)
        html.required.set(true)
    }

    val excludedClasses = listOf(
        "com/niacg/backend/service/JvmTlsClient*",
        "com/niacg/backend/service/JvmTlsClient\$*",
    )

    classDirectories.setFrom(
        sourceSets.main.get().output.asFileTree.matching {
            excludedClasses.forEach { exclude(it) }
        }
    )
}

tasks.register("checkCoverage") {
    group = "verification"
    description = "Verifies JaCoCo instruction coverage is at least 95%"
    dependsOn(tasks.jacocoTestReport)

    doLast {
        val reportFile = layout.buildDirectory.file("reports/jacoco/test/jacocoTestReport.xml").get().asFile
        if (!reportFile.exists()) {
            throw GradleException("JaCoCo XML report not found: $reportFile")
        }

        val dbf = javax.xml.parsers.DocumentBuilderFactory.newInstance()
        dbf.setFeature("http://apache.org/xml/features/nonvalidating/load-external-dtd", false)
        val root = dbf.newDocumentBuilder().parse(reportFile)

        val reportElement = root.getElementsByTagName("report").item(0) as org.w3c.dom.Element
        val counters = reportElement.getElementsByTagName("counter")
        var totalInstructions = 0
        var coveredInstructions = 0

        for (i in 0 until counters.length) {
            val counter = counters.item(i) as org.w3c.dom.Element
            if (counter.getAttribute("type") == "INSTRUCTION") {
                coveredInstructions = counter.getAttribute("covered").toInt()
                totalInstructions = counter.getAttribute("covered").toInt() +
                    counter.getAttribute("missed").toInt()
            }
        }

        val ratio = if (totalInstructions > 0) coveredInstructions.toDouble() / totalInstructions else 0.0
        val pct = (ratio * 100).let { "%.1f".format(it) }

        if (ratio < 0.95) {
            throw GradleException(
                "Coverage check FAILED: ${pct}% (${coveredInstructions}/${totalInstructions}) < 95%"
            )
        }

        logger.lifecycle("Coverage check PASSED: ${pct}% (${coveredInstructions}/${totalInstructions}) >= 95%")
    }
}

tasks.check {
    dependsOn(tasks.named("checkCoverage"))
}
