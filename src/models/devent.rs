use chrono::{DateTime, MappedLocalTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgHasArrayType, query, FromRow, PgPool, Type};
use uuid::Uuid;
use std::fmt;
use anyhow::{Result, Error};

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

impl fmt::Display for MouseAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MouseAction::Left => write!(f, "left"),
            MouseAction::Right => write!(f, "right"),
            MouseAction::Middle => write!(f, "middle"),
            MouseAction::Button4 => write!(f, "button4"),
            MouseAction::Button5 => write!(f, "button5"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "modifier_key_enum", rename_all = "lowercase")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub enum ModifierKey {
    #[sqlx(rename = "caps_lock")]
    #[serde(rename = "caps_lock")]
    CapsLock,
    Shift,
    Command,
    Option,
    Control,
    Fn,
    Alt,
    Meta,
}

impl PgHasArrayType for ModifierKey {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        <Self as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "keyboard_action_key_enum", rename_all = "lowercase")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub enum KeyboardActionKey {
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
    #[sqlx(rename = "0")]
    #[serde(rename = "0")]
    Num0,
    #[sqlx(rename = "1")]
    #[serde(rename = "1")]
    Num1,
    #[sqlx(rename = "2")]
    #[serde(rename = "2")]
    Num2,
    #[sqlx(rename = "3")]
    #[serde(rename = "3")]
    Num3,
    #[sqlx(rename = "4")]
    #[serde(rename = "4")]
    Num4,
    #[sqlx(rename = "5")]
    #[serde(rename = "5")]
    Num5,
    #[sqlx(rename = "6")]
    #[serde(rename = "6")]
    Num6,
    #[sqlx(rename = "7")]
    #[serde(rename = "7")]
    Num7,
    #[sqlx(rename = "8")]
    #[serde(rename = "8")]
    Num8,
    #[sqlx(rename = "9")]
    #[serde(rename = "9")]
    Num9,
    // Navigation Keys
    #[sqlx(rename = "arrow_up")]
    #[serde(rename = "arrow_up")]
    ArrowUp,
    #[sqlx(rename = "arrow_down")]
    #[serde(rename = "arrow_down")]
    ArrowDown,
    #[sqlx(rename = "arrow_left")]
    #[serde(rename = "arrow_left")]
    ArrowLeft,
    #[sqlx(rename = "arrow_right")]
    #[serde(rename = "arrow_right")]
    ArrowRight,
    Home,
    End,
    #[sqlx(rename = "page_up")]
    #[serde(rename = "page_up")]
    PageUp,
    #[sqlx(rename = "page_down")]
    #[serde(rename = "page_down")]
    PageDown,
    // Special Keys
    Escape,
    Enter,
    Tab,
    Space,
    Backspace,
    Insert,
    Delete,
    #[sqlx(rename = "num_lock")]
    #[serde(rename = "num_lock")]
    NumLock,
    #[sqlx(rename = "scroll_lock")]
    #[serde(rename = "scroll_lock")]
    ScrollLock,
    Pause,
    #[sqlx(rename = "print_screen")]
    #[serde(rename = "print_screen")]
    PrintScreen,
    // Symbols
    Grave,
    Minus,
    Equals,
    #[sqlx(rename = "bracket_left")]
    #[serde(rename = "bracket_left")]
    BracketLeft,
    #[sqlx(rename = "bracket_right")]
    #[serde(rename = "bracket_right")]
    BracketRight,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backslash,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "keyboard_action")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub struct KeyboardAction {
    pub key: KeyboardActionKey,
    pub modifiers: Vec<ModifierKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "scroll_action")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub struct ScrollAction {
    pub x: i32,
    pub y: i32,
    pub duration: i32,
}

// Desktop event, hence devent
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Devent {
    pub id: Uuid,
    pub session_id: Uuid,
    pub recording_id: Uuid,
    pub mouse_action: Option<MouseAction>,
    pub keyboard_action: Option<KeyboardAction>,
    pub scroll_action: Option<ScrollAction>,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub event_timestamp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Default for Devent {
    fn default() -> Self {
        Devent {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            recording_id: Uuid::new_v4(),
            mouse_action: None,
            keyboard_action: None,
            scroll_action: None,
            mouse_x: 0,
            mouse_y: 0,
            event_timestamp: Utc::now(),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Devent {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        pool: &PgPool,
        session_id: Uuid,
        recording_id: Uuid,
        mouse_action: Option<MouseAction>,
        keyboard_action: Option<KeyboardAction>,
        scroll_action: Option<ScrollAction>,
        mouse_x: i32,
        mouse_y: i32,
        event_timestamp: i64,
    ) -> Result<Self, Error> {
        let event_timestamp = match Utc.timestamp_opt(event_timestamp, 0) {
            MappedLocalTime::Single(et) => et,
            _ => return Err(anyhow::anyhow!("Invalid event_timestamp")),
        };

        let devent = Devent {
            id: Uuid::new_v4(),
            session_id,
            recording_id,
            mouse_action,
            keyboard_action,
            scroll_action,
            mouse_x,
            mouse_y,
            event_timestamp,
            ..Default::default()
        };

        query!(
            r#"
            INSERT INTO devents (id, session_id, recording_id, mouse_action, keyboard_action, scroll_action, mouse_x, mouse_y, event_timestamp, deleted_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
            devent.id,
            devent.session_id,
            devent.recording_id,
            devent.mouse_action.clone() as Option<MouseAction>,
            devent.keyboard_action.clone() as Option<KeyboardAction>,
            devent.scroll_action.clone() as Option<ScrollAction>,
            devent.mouse_x,
            devent.mouse_y,
            devent.event_timestamp,
            devent.deleted_at,
            devent.created_at,
            devent.updated_at
        )
        .execute(pool)
        .await?;

        Ok(devent)
    }

    pub async fn get(pool: &PgPool, id: Uuid) -> Result<Devent, Error> {
        let query_str = "SELECT * FROM devents WHERE id = $1";
        
        let devent = sqlx::query_as::<_, Devent>(query_str)
            .bind(id)
            .fetch_one(pool)
            .await?;

        Ok(devent)
    }

    pub async fn get_all_for_session(pool: &PgPool, session_id: Uuid) -> Result<Vec<Devent>, Error> {
        let query_str = "SELECT * FROM devents WHERE session_id = $1";

        let devents = sqlx::query_as::<_, Devent>(query_str)
            .bind(session_id)
            .fetch_all(pool)
            .await?;

        Ok(devents)
    }

    pub async fn get_all_for_recording(pool: &PgPool, recording_id: Uuid) -> Result<Vec<Devent>, Error> {
        let query_str = "SELECT * FROM devents WHERE recording_id = $1";

        let devents = sqlx::query_as::<_, Devent>(query_str)
            .bind(recording_id)
            .fetch_all(pool)
            .await?;

        Ok(devents)
    }
}