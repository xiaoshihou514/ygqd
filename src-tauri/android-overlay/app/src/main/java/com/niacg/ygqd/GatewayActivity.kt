package com.niacg.ygqd

import android.os.Bundle
import androidx.activity.OnBackPressedCallback

/**
 * 小说源网关专用 Activity(hlib 的 CF 验证/登录/抓取都在这里的 webview 里跑)。
 *
 * 本文件属于仓库 overlay:src-tauri/gen/android 由 `tauri android init` 整体生成且不入库,
 * `npm run android:init` 会在生成后执行 scripts/apply-android-overlay.mjs,把本文件拷进
 * gen 工程并在 AndroidManifest.xml 里登记本 Activity。改这里即可,勿改 gen 下的副本
 * (重新 init 会被覆盖回本文件)。
 *
 * 为什么要独立 Activity:wry 在 Android 上一个 Activity 只挂一个 webview——
 * eval/loadUrl 一律投给该 Activity「最后创建的 webview」。网关若与主界面同处
 * MainActivity,主界面的 Tauri IPC 回包(Android 上经 eval 回调)会全部投进网关,
 * 主界面所有 invoke 永不返回。独立 Activity 各有自己的消息管道,路由互不干扰。
 *
 * 生命周期约定(与 src-tauri/src/gateway.rs 对应):
 * - 创建:Rust 侧 WebviewWindowBuilder.activity_name("GatewayActivity") 经 tao 启动;
 *   清单里配独立 taskAffinity + singleTask,落在单独的任务栈
 * - 收回:Rust 调本 Activity 的 moveTaskToBack(true),只是把网关任务栈退到后台,
 *   Activity 与 webview 保活(不 finish——Rust 侧网关窗口对象需持续有效)
 * - 再次前置:Rust 以本类重新 startActivity,singleTask 走 onNewIntent,不重建
 * - 返回键:等同收回,回到主界面而不是销毁网关
 *
 * 必须继承 TauriActivity:tauri 建 webview 时会对所在 Activity 调 getPluginManager()
 * (manager/webview.rs),只继承 WryActivity 会 NoSuchMethodError 闪退(真机实测)。
 * PluginManager.onActivityCreate 只认第一个 Activity(已初始化即返回),插件仍绑定主界面。
 */
class GatewayActivity : TauriActivity() {
    // 不走默认返回键处理(网页后退,退到底就结束本 Activity)
    override val handleBackNavigation: Boolean = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    moveTaskToBack(true)
                }
            },
        )
    }
}
