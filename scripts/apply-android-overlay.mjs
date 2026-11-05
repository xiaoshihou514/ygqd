// 把仓库里的 Android overlay 应用到 `tauri android init` 生成的工程。
//
// 背景:src-tauri/gen/android 每次 init 整体重生成、且不入库(CI 也是现生成),
// 对生成工程的定制只能放在 src-tauri/android-overlay/ 下,由本脚本在 init 之后拷入。
// `npm run android:init` 已串上本脚本;脚本幂等,重复执行结果不变。
//
// 当前 overlay 内容:
// - GatewayActivity.kt:小说源网关的独立 Activity(原因见该文件头注释)
// - AndroidManifest.xml 里登记 GatewayActivity(独立任务栈、不进最近任务)
import { cp, readFile, writeFile } from 'node:fs/promises'

const root = new URL('../src-tauri/', import.meta.url)
const overlayDir = new URL('android-overlay/', root)
const genDir = new URL('gen/android/', root)
const manifestPath = new URL('app/src/main/AndroidManifest.xml', genDir)

let manifest
try {
  manifest = await readFile(manifestPath, 'utf8')
} catch (error) {
  throw new Error('Android project is missing; run `tauri android init` first.', { cause: error })
}

// 1. 源文件:overlay 目录树原样覆盖到 gen 工程
await cp(overlayDir, genDir, { recursive: true })

// 2. 清单登记(已登记则跳过)
const gatewayActivity = `        <activity
            android:name=".GatewayActivity"
            android:configChanges="orientation|keyboardHidden|keyboard|screenSize|locale|smallestScreenSize|screenLayout|uiMode"
            android:launchMode="singleTask"
            android:taskAffinity="\${applicationId}.gateway"
            android:excludeFromRecents="true"
            android:label="@string/main_activity_title"
            android:exported="false" />
`
if (!manifest.includes('.GatewayActivity"')) {
  const marker = '    </application>'
  if (!manifest.includes(marker)) {
    throw new Error('Could not find </application> in AndroidManifest.xml.')
  }
  manifest = manifest.replace(marker, `${gatewayActivity}${marker}`)
  await writeFile(manifestPath, manifest)
}

console.log('Android overlay applied (GatewayActivity).')
