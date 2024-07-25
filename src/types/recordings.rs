use serde::Deserialize;
use uuid::Uuid;

use super::Timestamp;

#[derive(Deserialize)]
pub struct SaveRecordingRequest {
    pub recording_id: Uuid,
    pub session_id: Uuid,
    pub start_timestamp: Timestamp,
    pub duration_ms: u64,
}