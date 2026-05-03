package com.niacg.backend.server

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.ViewGroup
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
            text = if (BackendService.isRunning) "● Running" else "○ Stopped"
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
            webViewClient = WebViewClient()
            loadUrl("http://localhost:${BackendService.DEFAULT_PORT}/")
        }
        rootLayout.addView(webView)

        setContentView(rootLayout)
    }

    private fun toggleServer() {
        if (BackendService.isRunning) {
            stopService(Intent(this, BackendService::class.java))
            BackendService.isRunning = false
            statusText?.text = "○ Stopped"
            toggleBtn?.text = "Start"
        } else {
            val intent = Intent(this, BackendService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(intent)
            } else {
                startService(intent)
            }
            statusText?.text = "● Running"
            toggleBtn?.text = "Stop"

            webView?.postDelayed({
                webView?.loadUrl("http://localhost:${BackendService.DEFAULT_PORT}/")
            }, 500)
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
