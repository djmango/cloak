use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SaveRecordingRequest {
    pub recording_id: Uuid,
    pub session_id: Uuid,
    pub start_timestamp: i64,
}