use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, FromRow, PgPool, Type};
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

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
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

impl Devent {
    pub async fn new(
        pool: &PgPool,
        session_id: Uuid,
        mouse_action: Option<MouseAction>,
        keyboard_action: Option<KeyboardAction>,
        scroll_action: Option<ScrollAction>,
        mouse_x: i32,
        mouse_y: i32,
        event_timestamp: i64,
    ) -> Result<Self, Error> {
        let event_timestamp = chrono::DateTime::from_timestamp(event_timestamp, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid event_timestamp"))?
            .naive_utc();

        let devent = Devent {
            id: Uuid::new_v4(),
            session_id,
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
            INSERT INTO devents (id, session_id, mouse_action, keyboard_action, scroll_action, mouse_x, mouse_y, event_timestamp, deleted_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            devent.id,
            devent.session_id,
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
}