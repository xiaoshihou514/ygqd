package com.niacg.backend.server

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.widget.Button
import android.widget.TextView
import android.app.Activity

class MainActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val layout = android.widget.LinearLayout(this).apply {
            orientation = android.widget.LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
        }

        layout.addView(TextView(this).apply {
            text = "Niacg Kotlin Backend"
            textSize = 20f
        })

        layout.addView(TextView(this).apply {
            text = "API → http://localhost:${BackendService.DEFAULT_PORT}/api/"
            textSize = 14f
            setPadding(0, 16, 0, 0)
        })

        layout.addView(TextView(this).apply {
            text = "Endpoints: /home, /list, /search, /comic, /image, /split"
            textSize = 12f
            setPadding(0, 8, 0, 0)
        })

        val statusText = TextView(this).apply {
            text = if (BackendService.isRunning) "● Running" else "○ Stopped"
            textSize = 16f
            setPadding(0, 24, 0, 0)
        }
        layout.addView(statusText)

        val toggleBtn = Button(this).apply {
            text = if (BackendService.isRunning) "Stop Server" else "Start Server"
            setOnClickListener {
                if (BackendService.isRunning) {
                    stopService(Intent(this@MainActivity, BackendService::class.java))
                    BackendService.isRunning = false
                    statusText.text = "○ Stopped"
                    text = "Start Server"
                } else {
                    val intent = Intent(this@MainActivity, BackendService::class.java)
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        startForegroundService(intent)
                    } else {
                        startService(intent)
                    }
                    statusText.text = "● Running"
                    text = "Stop Server"
                }
            }
        }
        layout.addView(toggleBtn)

        setContentView(layout)
    }
}
