use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "scroll_action_enum", rename_all = "lowercase")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub enum ScrollAction {
    Up,
    Down,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "mouse_action_enum", rename_all = "lowercase")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub enum MouseAction {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "keyboard_action_enum", rename_all = "lowercase")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub enum KeyboardAction {
    // Modifier Keys
    Shift,
    Control,
    Alt,
    Meta,
    // Function Keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    // Alphabet Keys
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Number Keys
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    // Navigation Keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    // Special Keys
    Escape,
    Enter,
    Tab,
    Space,
    Backspace,
    Insert,
    Delete,
    CapsLock,
    NumLock,
    ScrollLock,
    Pause,
    PrintScreen,
}

// Desktop event, hence devent

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Devent {
    pub id: Uuid,
    pub session_id: Uuid,
    pub recording_id: Option<Uuid>,
    pub mouse_action: Option<MouseAction>,
    pub keyboard_action: Option<KeyboardAction>,
    pub scroll_action: Option<ScrollAction>,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub event_timestamp: chrono::NaiveDateTime,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Default for Devent {
    fn default() -> Self {
        Devent {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            recording_id: None,
            mouse_action: None,
            keyboard_action: None,
            scroll_action: None,
            mouse_x: 0,
            mouse_y: 0,
            event_timestamp: chrono::NaiveDateTime::default(),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
