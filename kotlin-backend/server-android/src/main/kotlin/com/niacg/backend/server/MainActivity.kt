package com.niacg.backend.server

import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.View
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.app.Activity
import android.view.ViewGroup

class MainActivity : Activity() {

    private var webView: WebView? = null
    private var retryCount = 0
    private val maxRetries = 20

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

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

                override fun onReceivedError(
                    view: WebView?,
                    request: WebResourceRequest?,
                    error: WebResourceError?
                ) {
                    if (request?.isForMainFrame == true && retryCount < maxRetries) {
                        retryCount++
                        view?.postDelayed({
                            if (BackendService.isRunning) {
                                view?.loadUrl("http://localhost:${BackendService.DEFAULT_PORT}/")
                            }
                        }, 1000)
                    }
                }
            }
        }
        setContentView(webView)

        ensureServerAndLoad()
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
        webView?.loadUrl("http://localhost:${BackendService.DEFAULT_PORT}/")
    }

    override fun onBackPressed() {
        if (webView?.canGoBack() == true) {
            webView?.goBack()
        } else {
            super.onBackPressed()
        }
    }
}
