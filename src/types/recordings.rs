use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SaveRecordingRequest {
    pub session_id: Uuid,
    pub start_timestamp: i64,
}