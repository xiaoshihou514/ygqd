package com.niacg.backend.server

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import com.niacg.backend.service.AndroidHttpClient
import io.ktor.server.cio.CIO
import io.ktor.server.engine.EmbeddedServer
import io.ktor.server.engine.embeddedServer

class BackendService : Service() {

    companion object {
        const val CHANNEL_ID = "niacg_backend_channel"
        const val NOTIFICATION_ID = 1001
        const val DEFAULT_PORT = 8080

        var serverInstance: EmbeddedServer<*, *>? = null
            private set

        var isRunning = false
            private set
    }

    private var server: EmbeddedServer<*, *>? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notification = createNotification()
        startForeground(NOTIFICATION_ID, notification)

        val port = intent?.getIntExtra("port", DEFAULT_PORT) ?: DEFAULT_PORT

        if (!isRunning) {
            startServer(port)
        }

        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        stopServer()
        super.onDestroy()
    }

    private fun startServer(port: Int) {
        val httpClient = AndroidHttpClient()

        server = embeddedServer(CIO, port = port, host = "0.0.0.0") {
            module(httpClient)
        }.start(wait = false)

        serverInstance = server
        isRunning = true
    }

    private fun stopServer() {
        server?.stop(1000, 2000)
        server = null
        serverInstance = null
        isRunning = false
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Niacg Backend",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Niacg backend server running"
            }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun createNotification(): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this, 0, packageManager
                .getLaunchIntentForPackage(packageName)
                ?.apply { flags = Intent.FLAG_ACTIVITY_SINGLE_TOP },
            PendingIntent.FLAG_IMMUTABLE
        )

        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("Niacg Backend")
                .setContentText("Server running on port $DEFAULT_PORT")
                .setSmallIcon(android.R.drawable.ic_menu_manage)
                .setContentIntent(pendingIntent)
                .setOngoing(true)
                .build()
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
                .setContentTitle("Niacg Backend")
                .setContentText("Server running on port $DEFAULT_PORT")
                .setSmallIcon(android.R.drawable.ic_menu_manage)
                .setContentIntent(pendingIntent)
                .setOngoing(true)
                .build()
        }
    }
}
