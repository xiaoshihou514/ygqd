package com.niacg.backend.server

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.app.Activity

class MainActivity : Activity() {

    private var webView: WebView? = null
    private var retryCount = 0
    private val maxRetries = 20

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        webView = WebView(this).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = false
            webViewClient = object : WebViewClient() {
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
