use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Chat {
    pub id: Uuid,
    pub user_id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Chat {
    pub fn new(user_id: &str, name: &str) -> Self {
        Chat {
            user_id: user_id.to_string(),
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Returns a chat for a given user_id, if it exists, otherwise creates a new chat and returns it.
    pub async fn get_or_create_by_user_id_and_chat_id(
        pool: &PgPool,
        user_id: &str,
        chat_id: Option<Uuid>,
    ) -> Result<Self> {
        // Fetch by chat_id if it exists
        if let Some(chat_id) = chat_id {
            if let Some(chat) = query_as!(
                Chat,
                r#"
                SELECT * FROM chats 
                WHERE id = $1
                "#,
                chat_id
            )
            .fetch_optional(pool)
            .await?
            {
                debug!("Chat found: {:?}", chat);
                return Ok(chat);
            }
        }

        // Otherwise, fetch by user_id
        if let Some(chat) = query_as!(
            Chat,
            r#"
            SELECT * FROM chats 
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await?
        {
            debug!("Chat found: {:?}", chat);
            return Ok(chat);
        }

        // Otherwise, create a new chat
        let chat = Chat::new(user_id, "New Chat");
        let chat = query_as!(
            Chat,
            r#"
                INSERT INTO chats (id, user_id, name, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING *
                "#,
            chat.id,
            chat.user_id,
            chat.name,
            chat.created_at,
            chat.updated_at
        )
        .fetch_one(pool)
        .await?;

        debug!("Chat created: {:?}", chat);
        Ok(chat)
    }
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            id: Uuid::new_v4(),
            user_id: String::new(),
            name: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
