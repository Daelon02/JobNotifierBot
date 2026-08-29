use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEmailNotification {
    pub message_id: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub snippet: String,
}
