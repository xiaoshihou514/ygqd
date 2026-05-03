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
import java.io.File

class BackendService : Service() {

    companion object {
        const val CHANNEL_ID = "niacg_backend_channel"
        const val NOTIFICATION_ID = 1001
        const val DEFAULT_PORT = 8080
        const val WEB_ASSETS_DIR = "web"

        var serverInstance: EmbeddedServer<*, *>? = null
            private set

        var isRunning = false
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
            val webDir = prepareWebAssets()
            startServer(port, webDir)
        }

        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        stopServer()
        super.onDestroy()
    }

    private fun prepareWebAssets(): File? {
        return try {
            val webDir = File(filesDir, WEB_ASSETS_DIR)
            if (!webDir.exists()) {
                webDir.mkdirs()
                copyAssets(assets, WEB_ASSETS_DIR, webDir)
            }
            webDir
        } catch (e: Exception) {
            null
        }
    }

    private fun copyAssets(am: android.content.res.AssetManager, path: String, target: File) {
        val list = am.list(path) ?: return
        for (name in list) {
            val childPath = if (path.isEmpty()) name else "$path/$name"
            val childTarget = File(target, name)

            val subList = am.list(childPath)
            if (subList != null && subList.isNotEmpty()) {
                childTarget.mkdirs()
                copyAssets(am, childPath, childTarget)
            } else {
                am.open(childPath).use { input ->
                    childTarget.outputStream().use { output ->
                        input.copyTo(output)
                    }
                }
            }
        }
    }

    private fun startServer(port: Int, webDir: File?) {
        val httpClient = AndroidHttpClient()

        server = embeddedServer(CIO, port = port, host = "0.0.0.0") {
            module(httpClient, webDir)
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
