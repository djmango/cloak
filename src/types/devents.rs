use serde::Deserialize;
use uuid::Uuid;

use crate::models::devent::{KeyboardAction, MouseAction, ScrollAction};

#[derive(Deserialize)]
pub struct CreateDeventRequest {
    pub session_id: Uuid,
    pub recording_id: Uuid,
    pub mouse_action: Option<MouseAction>,
    pub keyboard_action: Option<KeyboardAction>,
    pub scroll_action: Option<ScrollAction>,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub event_timestamp: i64,
}