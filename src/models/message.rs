use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "role_enum")] // SQL type name
#[sqlx(rename_all = "lowercase")] // SQL value name
pub enum Role {
    Assistant,
    System,
    Tool,
    User,
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub user_id: String,
    pub text: String,
    pub role: Role,
    pub files: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Message {
    pub fn new(chat_id: Uuid, user_id: &str, text: &str, role: Role) -> Self {
        Message {
            chat_id,
            user_id: user_id.to_string(),
            text: text.to_string(),
            role,
            ..Default::default()
        }
    }
}

impl Default for Message {
    fn default() -> Self {
        Message {
            id: Uuid::new_v4(),
            chat_id: Uuid::nil(),
            user_id: String::new(),
            text: String::new(),
            role: Role::User,
            files: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
