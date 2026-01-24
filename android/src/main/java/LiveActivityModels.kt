package com.astralsight.astrobox.plugin.live_activity

import app.tauri.annotation.InvokeArg

@InvokeArg
class CreateLiveActivityArgs {
    var activity_content_v: Int = 0

    var activity_content: ActivityContent? = null
}

@InvokeArg
class ActivityContent {
    var type: String? = null
    var data: ActivityContentTaskQueue? = null
}

@InvokeArg
class ActivityContentTaskQueue {
    var id: String? = null
    var title: String? = null
    var text: String? = null

    var taskName: String? = null

    var taskType: String? = null

    var taskIcon: String? = null

    var state: Map<String, String>? = null
}

@InvokeArg
class UpdateLiveActivityArgs {
    var state: Map<String, String>? = null
}
