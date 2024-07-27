use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SaveRecordingRequest {
    pub recording_id: Uuid,
    pub session_id: Uuid,
    pub start_timestamp_nanos: i64,
    pub duration_ms: u64,
}