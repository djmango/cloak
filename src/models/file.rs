use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "filetype_enum", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Filetype {
    Jpeg,
    Pdf,
    Mp4,
    Mp3,
}

#[derive(Debug, FromRow, Serialize, Deserialize, ToSchema)]
pub struct File {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub user_id: String,
    pub message_id: Uuid,
    pub filetype: Filetype,
    pub show_to_user: bool,
    pub url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl File {
    pub fn new(
        chat_id: Uuid,
        user_id: &str,
        message_id: Uuid,
        filetype: Filetype,
        show_to_user: bool,
        url: Option<String>,
    ) -> Self {
        Self {
            chat_id,
            user_id: user_id.to_string(),
            message_id,
            filetype,
            show_to_user,
            url,
            ..Default::default()
        }
    }
}

impl Default for File {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            chat_id: Uuid::nil(),
            user_id: String::new(),
            message_id: Uuid::nil(),
            filetype: Filetype::Jpeg,
            show_to_user: false,
            url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
