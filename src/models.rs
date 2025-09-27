use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct ActivityContentTaskQueue {
    pub id: String,
    pub title: String,
    pub text: String,
    #[serde(rename = "taskName")]
    pub task_name: String,
    #[serde(rename = "taskType")]
    pub task_type: String,
    #[serde(rename = "taskIcon")]
    pub task_icon: String,
    pub state: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ActivityContentTaskQueueBasicState {
    pub percent: String,
    pub progress: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ActivityContent {
    TaskQueue(ActivityContentTaskQueue),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateLiveActivityRequest {
    pub activity_content_v: u32,
    pub activity_content: ActivityContent,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateLiveActivityRequest {
    pub state: HashMap<String, String>,
}
