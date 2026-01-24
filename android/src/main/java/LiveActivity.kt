package com.astralsight.astrobox.plugin.live_activity

import android.Manifest
import android.app.Activity
import androidx.annotation.RequiresPermission
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@TauriPlugin
class LiveActivity(private val activity: Activity) : Plugin(activity) {
    private val manager = LiveActivityManager(activity)

    @Command
    fun createLiveActivity(invoke: Invoke) {
        val args = invoke.parseArgs(CreateLiveActivityArgs::class.java)
        manager.create(args)
        invoke.resolve()
    }

    @RequiresPermission(Manifest.permission.POST_NOTIFICATIONS)
    @Command
    fun updateLiveActivity(invoke: Invoke) {
        val args = invoke.parseArgs(UpdateLiveActivityArgs::class.java)
        manager.update(args)
        invoke.resolve()
    }

    @Command
    fun removeLiveActivity(invoke: Invoke) {
        manager.remove()
        invoke.resolve()
    }
}
