use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct UpvoteMessageRequest {
    pub message_id: Uuid,
}

#[derive(Deserialize)]
pub struct DownvoteMessageRequest {
    pub message_id: Uuid,
}