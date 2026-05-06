package com.niacg.backend.server

import android.webkit.JavascriptInterface

class AndroidBridge {
    @JavascriptInterface
    fun isAndroidApp(): Boolean = true
}
