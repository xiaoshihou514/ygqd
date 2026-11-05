package com.niacg.ygqd

import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.activity.OnBackPressedCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

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

    // 安全区包裹是否已完成(WebView 异步挂载,轮询等待后只做一次)
    private var safeAreaWrapped = false

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
        setupGatewaySafeArea()
    }

    /**
     * 网关安全区适配:tauri Android 默认 edge-to-edge,WebView 内容延伸到
     * 系统栏下——hlib 页面顶栏被状态栏遮挡(完成栏已移底,顶部无 fixed
     * 元素)。WebView 本体忽略自身 padding,不能直接 setPadding;正确
     * 做法是把框架挂载的 WebView 从父容器移出,包进带顶部 padding 的
     * FrameLayout,并监听 WindowInsets 动态适配状态栏高度(跨设备)。
     * 状态栏保持透明(edge-to-edge 标准做法),视觉融合不另涂色。
     *
     * WebView 由 tauri 在 onCreate 之后异步挂载,轮询等待其出现在视图树。
     */
    private fun setupGatewaySafeArea() {
        // 状态栏保持透明(edge-to-edge 标准做法):完成栏已移底,顶部无
        // fixed 元素,透明状态栏让 hlib 页面顶栏自然延伸到系统栏下
        WindowCompat.setDecorFitsSystemWindows(window, false)
        pollWrapWebView()
    }

    private fun pollWrapWebView(attempt: Int = 0) {
        if (safeAreaWrapped) return
        val webview = findWebView(window.decorView)
        if (webview == null) {
            if (attempt < 25) {
                window.decorView.postDelayed({ pollWrapWebView(attempt + 1) }, 200)
            } else {
                android.util.Log.w("GatewayActivity", "WebView 挂载轮询超时,安全区适配放弃(慢设备兜底)")
            }
            return
        }
        wrapWebViewWithInsets(webview)
        safeAreaWrapped = true
    }

    private fun findWebView(root: View): WebView? {
        if (root is WebView) return root
        if (root is ViewGroup) {
            for (i in 0 until root.childCount) {
                findWebView(root.getChildAt(i))?.let { return it }
            }
        }
        return null
    }

    private fun wrapWebViewWithInsets(webview: WebView) {
        val parent = webview.parent as? ViewGroup ?: return
        val index = parent.indexOfChild(webview)
        val lp = webview.layoutParams
        parent.removeView(webview)

        val wrapper = FrameLayout(this)
        parent.addView(wrapper, index, lp)
        wrapper.addView(
            webview,
            ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )

        // 动态适配系统栏高度:不消费 insets,让子视图照常收到
        ViewCompat.setOnApplyWindowInsetsListener(wrapper) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.systemBars()).top
            v.setPadding(0, top, 0, 0)
            insets
        }
        ViewCompat.requestApplyInsets(wrapper)
    }
}
