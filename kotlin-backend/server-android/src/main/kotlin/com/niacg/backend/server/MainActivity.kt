package com.niacg.backend.server

import android.app.Activity
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient

class MainActivity : Activity() {

    private var webView: WebView? = null
    private var retryCount = 0
    private val maxRetries = 20
    private var baseUrl: String = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        baseUrl = "http://localhost:${BackendService.DEFAULT_PORT}"

        setupStatusBar()

        webView = WebView(this).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = false
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                    injectAndroidEnv(view)
                }

                override fun shouldOverrideUrlLoading(
                    view: WebView?,
                    request: WebResourceRequest?
                ): Boolean {
                    val url = request?.url ?: return false
                    if (handleInternalNavigation(url)) {
                        return true
                    }
                    return false
                }

                @Deprecated("Deprecated in Java")
                override fun shouldOverrideUrlLoading(
                    view: WebView?,
                    url: String?
                ): Boolean {
                    if (url != null && handleInternalNavigation(Uri.parse(url))) {
                        return true
                    }
                    return false
                }

                override fun onReceivedError(
                    view: WebView?,
                    request: WebResourceRequest?,
                    error: WebResourceError?
                ) {
                    if (request?.isForMainFrame == true && retryCount < maxRetries) {
                        retryCount++
                        view?.postDelayed({
                            if (BackendService.isRunning) {
                                view?.loadUrl("$baseUrl/")
                            }
                        }, 1000)
                    }
                }
            }
        }
        setContentView(webView)

        ensureServerAndLoad()
    }

    private fun handleInternalNavigation(uri: Uri): Boolean {
        val host = uri.host ?: return false
        val scheme = uri.scheme ?: return false

        if (scheme == "http" || scheme == "https") {
            if (host == "localhost" || host == "127.0.0.1") {
                val path = uri.path ?: "/"
                val query = uri.query
                val routePath = if (query != null) "$path?$query" else path

                webView?.evaluateJavascript(
                    "window.__VUE_ROUTER__ && window.__VUE_ROUTER__.push('$routePath')",
                    null
                )
                return true
            }
        }
        return false
    }

    override fun onDestroy() {
        super.onDestroy()
        if (BackendService.isRunning) {
            stopService(Intent(this, BackendService::class.java))
            BackendService.isRunning = false
        }
    }

    private fun setupStatusBar() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            window.statusBarColor = Color.parseColor("#000000")
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
        }
    }

    private fun isSystemDark(): Boolean {
        val flags = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
        return flags == Configuration.UI_MODE_NIGHT_YES
    }

    private fun injectAndroidEnv(view: WebView?) {
        val dark = isSystemDark()
        view?.evaluateJavascript("""
            (function(){
                document.documentElement.classList.add('android');
                window.__ANDROID_DARK_MODE__ = $dark;
                window.dispatchEvent(new CustomEvent('android-ready'));
            })()
        """.trimIndent(), null)
    }

    private fun ensureServerAndLoad() {
        if (BackendService.isRunning) {
            loadWebView()
        } else {
            val intent = Intent(this, BackendService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(intent)
            } else {
                startService(intent)
            }
            scheduleServerPoll()
        }
    }

    private fun scheduleServerPoll() {
        webView?.postDelayed({
            if (BackendService.isRunning) {
                loadWebView()
            } else if (retryCount < maxRetries) {
                retryCount++
                scheduleServerPoll()
            }
        }, 500)
    }

    private fun loadWebView() {
        retryCount = 0
        webView?.loadUrl("$baseUrl/")
    }

    override fun onBackPressed() {
        webView?.evaluateJavascript(
            "window.__VUE_ROUTER__ && window.__VUE_ROUTER__.back()",
            null
        )
    }
}
