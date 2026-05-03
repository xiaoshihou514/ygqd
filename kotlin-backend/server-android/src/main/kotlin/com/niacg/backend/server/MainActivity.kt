package com.niacg.backend.server

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import android.app.Activity

class MainActivity : Activity() {

    private var webView: WebView? = null
    private var statusText: TextView? = null
    private var toggleBtn: Button? = null
    private var retryCount = 0
    private val maxRetries = 20

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val rootLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }

        val toolbar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(16, 16, 16, 8)
        }

        statusText = TextView(this).apply {
            text = if (BackendService.isRunning) "● Running" else "○ Starting..."
            textSize = 14f
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        }
        toolbar.addView(statusText)

        toggleBtn = Button(this).apply {
            text = if (BackendService.isRunning) "Stop" else "Start"
            textSize = 12f
            setPadding(24, 8, 24, 8)
            setOnClickListener {
                toggleServer()
            }
        }
        toolbar.addView(toggleBtn)

        rootLayout.addView(toolbar)

        webView = WebView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                0,
                1f
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
        rootLayout.addView(webView)

        setContentView(rootLayout)

        ensureServerAndLoad()
    }

    override fun onResume() {
        super.onResume()
        if (BackendService.isRunning && webView?.url == null) {
            loadWebView()
        }
    }

    private fun ensureServerAndLoad() {
        if (BackendService.isRunning) {
            updateStatusUI()
            loadWebView()
        } else {
            val intent = Intent(this, BackendService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(intent)
            } else {
                startService(intent)
            }
            statusText?.text = "● Starting..."
            toggleBtn?.text = "Stop"
            scheduleServerPoll()
        }
    }

    private fun scheduleServerPoll() {
        webView?.postDelayed({
            if (BackendService.isRunning) {
                updateStatusUI()
                loadWebView()
            } else if (retryCount < maxRetries) {
                retryCount++
                scheduleServerPoll()
            }
        }, 500)
    }

    private fun updateStatusUI() {
        statusText?.text = if (BackendService.isRunning) "● Running" else "○ Stopped"
        toggleBtn?.text = if (BackendService.isRunning) "Stop" else "Start"
    }

    private fun loadWebView() {
        retryCount = 0
        webView?.loadUrl("http://localhost:${BackendService.DEFAULT_PORT}/")
    }

    private fun toggleServer() {
        if (BackendService.isRunning) {
            stopService(Intent(this, BackendService::class.java))
            BackendService.isRunning = false
            webView?.loadUrl("about:blank")
            updateStatusUI()
        } else {
            retryCount = 0
            statusText?.text = "● Starting..."
            toggleBtn?.text = "Stop"
            val intent = Intent(this, BackendService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(intent)
            } else {
                startService(intent)
            }
            scheduleServerPoll()
        }
    }

    override fun onBackPressed() {
        if (webView?.canGoBack() == true) {
            webView?.goBack()
        } else {
            super.onBackPressed()
        }
    }
}
