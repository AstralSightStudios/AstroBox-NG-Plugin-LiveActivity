package com.astralsight.astrobox.plugin.live_activity

import android.Manifest
import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.annotation.RequiresPermission
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import kotlin.math.roundToInt

private const val TAG = "LiveActivity"
private const val CHANNEL_ID = "live_activity"
private const val CHANNEL_NAME = "Live Activity"
private const val NOTIFICATION_ID = 9901
private const val NOTIFICATION_PERMISSION_REQUEST_CODE = 1001

class LiveActivityManager(private val activity: Activity) {
    private val notificationManager = NotificationManagerCompat.from(activity)
    private var current: LiveActivityData? = null
    private var isEnding = false

    data class LiveActivityData(
        val id: String,
        val title: String,
        val text: String,
        val taskName: String,
        val taskType: String,
        val taskIcon: String,
        var state: Map<String, String>
    )

    @RequiresPermission(Manifest.permission.POST_NOTIFICATIONS)
    fun create(args: CreateLiveActivityArgs) {
        if (current != null) {
            Log.i(TAG, "Live activity already exists; skip create.")
            return
        }

        if (!hasNotificationPermission()) {
            Log.w(TAG, "Notification permission not granted. Requesting permission.")
            requestNotificationPermission()
            return
        }

        try {
            val content = args.activity_content
            if (content?.type != "TaskQueue") {
                Log.i(TAG, "Unsupported live activity content type: ${content?.type}")
                return
            }

            val data = content.data
            if (data == null) {
                Log.i(TAG, "Missing live activity data payload.")
                return
            }

            val state = data.state ?: emptyMap()
            current = LiveActivityData(
                id = data.id ?: "",
                title = data.title ?: "",
                text = data.text ?: "",
                taskName = data.taskName ?: "",
                taskType = data.taskType ?: "",
                taskIcon = data.taskIcon ?: "",
                state = state
            )

            ensureChannel()
            notificationManager.notify(NOTIFICATION_ID, buildNotification(state))
            Log.i(TAG, "Live activity created.")
        } catch (e: SecurityException) {
            Log.e(TAG, "SecurityException while creating notification: ${e.message}")
            throw e
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create live activity: ${e.message}")
            throw e
        }
    }

    @RequiresPermission(Manifest.permission.POST_NOTIFICATIONS)
    fun update(args: UpdateLiveActivityArgs) {
        if (isEnding) {
            Log.i(TAG, "Live activity is ending; skip update.")
            return
        }

        val live = current
        if (live == null) {
            Log.i(TAG, "No live activity to update.")
            return
        }

        if (!hasNotificationPermission()) {
            Log.w(TAG, "Notification permission not granted. Cannot update live activity.")
            return
        }

        try {
            val state = args.state ?: live.state
            live.state = state
            ensureChannel()
            notificationManager.notify(NOTIFICATION_ID, buildNotification(state))
            Log.i(TAG, "Live activity updated.")
        } catch (e: SecurityException) {
            Log.e(TAG, "SecurityException while updating notification: ${e.message}")
            throw e
        } catch (e: Exception) {
            Log.e(TAG, "Failed to update live activity: ${e.message}")
            throw e
        }
    }

    fun remove() {
        if (current == null) {
            Log.i(TAG, "No live activity to remove.")
            return
        }

        try {
            isEnding = true
            notificationManager.cancel(NOTIFICATION_ID)
            current = null
            isEnding = false
            Log.i(TAG, "Live activity removed successfully.")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to remove live activity: ${e.message}")
            throw e
        }
    }

    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val existingChannel = notificationManager.getNotificationChannel(CHANNEL_ID)
            if (existingChannel == null) {
                val channel = NotificationChannel(
                    CHANNEL_ID,
                    CHANNEL_NAME,
                    NotificationManager.IMPORTANCE_HIGH
                )
                channel.description = "Live activity notifications for ongoing tasks"
                channel.enableLights(true)
                channel.enableVibration(true)
                channel.setShowBadge(true)
                channel.lockscreenVisibility = Notification.VISIBILITY_PUBLIC
                notificationManager.createNotificationChannel(channel)
                Log.d(TAG, "Notification channel created with HIGH importance: $CHANNEL_ID")
            } else {
                Log.d(TAG, "Notification channel already exists: $CHANNEL_ID")
            }
        }
    }

    private fun hasNotificationPermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.POST_NOTIFICATIONS
            ) == PackageManager.PERMISSION_GRANTED
        } else {
            true
        }
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ActivityCompat.requestPermissions(
                activity,
                arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                NOTIFICATION_PERMISSION_REQUEST_CODE
            )
            Log.i(TAG, "Notification permission requested")
        }
    }

    private fun buildNotification(state: Map<String, String>): Notification {
        val live = current ?: run {
            Log.w(TAG, "No live activity data available for building notification")
            return NotificationCompat.Builder(activity, CHANNEL_ID).build()
        }
        
        Log.d(TAG, "Building notification for activity: ${live.id}, state: $state")
        
        val iconRes = activity.applicationInfo.icon.takeIf { it != 0 }
            ?: android.R.drawable.ic_dialog_info

        val progressInfo = parseProgress(state)
        val contentText = buildContentText(live, progressInfo)

        val contentTitle = live.title.ifBlank { "Live Activity" }
        val builder = NotificationCompat.Builder(activity, CHANNEL_ID)
            .setSmallIcon(iconRes)
            .setContentTitle(contentTitle)
            .setContentText(contentText)
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setShortCriticalText("${progressInfo.percent}%")
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setWhen(System.currentTimeMillis())
            .setShowWhen(false)

        if (Build.VERSION.SDK_INT >= 36) {
            builder.setRequestPromotedOngoing(true)
            val style = NotificationCompat.ProgressStyle()
                .setProgress(progressInfo.percent)
                .setProgressIndeterminate(progressInfo.indeterminate)
            builder.setStyle(style)
            Log.d(TAG, "Using Android 36+ progress style: ${progressInfo.percent}%")
        } else {
            builder.setProgress(100, progressInfo.percent, progressInfo.indeterminate)
            Log.d(TAG, "Using legacy progress style: ${progressInfo.percent}%")
        }

        return builder.build()
    }

    private fun buildContentText(
        live: LiveActivityData,
        progressInfo: ProgressInfo
    ): String {
        val taskInfo = live.taskName.ifBlank { live.text }.ifBlank { live.taskType }
        return if (progressInfo.indeterminate) {
            if (taskInfo.isNotBlank()) taskInfo else live.text
        } else {
            val percentText = "${progressInfo.percent}%"
            if (taskInfo.isNotBlank()) "$taskInfo · $percentText" else percentText
        }
    }

    private fun parseProgress(state: Map<String, String>): ProgressInfo {
        val progressRaw = state["progress"]?.trim()
        val percentRaw = state["percent"]?.trim()?.removeSuffix("%")

        Log.d(TAG, "Parsing progress from state - progressRaw: $progressRaw, percentRaw: $percentRaw")

        if (!progressRaw.isNullOrBlank()) {
            val value = progressRaw.toFloatOrNull()
            if (value != null) {
                val percent = (value * 100f).roundToInt().coerceIn(0, 100)
                Log.d(TAG, "Progress parsed from 'progress' field: $percent%")
                return ProgressInfo(percent, false)
            }
        }

        if (!percentRaw.isNullOrBlank()) {
            val value = percentRaw.toFloatOrNull()
            if (value != null) {
                val percent = value.roundToInt().coerceIn(0, 100)
                Log.d(TAG, "Progress parsed from 'percent' field: $percent%")
                return ProgressInfo(percent, false)
            }
        }

        Log.d(TAG, "No valid progress found, using indeterminate progress")
        return ProgressInfo(0, true)
    }

    data class ProgressInfo(
        val percent: Int,
        val indeterminate: Boolean
    )
}
