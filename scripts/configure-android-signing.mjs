import { readFile, writeFile } from 'node:fs/promises'

const gradlePath = new URL('../src-tauri/gen/android/app/build.gradle.kts', import.meta.url)
let source

try {
  source = await readFile(gradlePath, 'utf8')
} catch (error) {
  throw new Error('Android project is missing; run `npm run android:init` first.', { cause: error })
}

if (!source.includes('import java.io.FileInputStream')) {
  source = source.replace(
    'import java.util.Properties',
    'import java.util.Properties\nimport java.io.FileInputStream',
  )
}

if (!source.includes('create("release")')) {
  const buildTypesMarker = '    buildTypes {'
  const signingConfig = `    signingConfigs {
        create("release") {
            val propertiesFile = rootProject.file("keystore.properties")
            val properties = Properties()
            if (!propertiesFile.exists()) {
                throw GradleException("Missing keystore.properties for release signing")
            }
            FileInputStream(propertiesFile).use { properties.load(it) }
            keyAlias = properties.getProperty("keyAlias")
            keyPassword = properties.getProperty("keyPassword")
            storeFile = file(properties.getProperty("storeFile"))
            storePassword = properties.getProperty("storePassword")
        }
    }

`

  if (!source.includes(buildTypesMarker)) {
    throw new Error('Could not find the Android buildTypes block.')
  }
  source = source.replace(buildTypesMarker, `${signingConfig}${buildTypesMarker}`)
}

const releaseMarker = '        getByName("release") {'
const signedReleaseMarker = `${releaseMarker}\n            signingConfig = signingConfigs.getByName("release")`
if (!source.includes(signedReleaseMarker)) {
  if (!source.includes(releaseMarker)) {
    throw new Error('Could not find the Android release build type.')
  }
  source = source.replace(releaseMarker, signedReleaseMarker)
}

await writeFile(gradlePath, source)
