use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;
use uuid::Uuid;

use crate::AppState;

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
    pub async fn get_or_create_by_user_id(pool: &PgPool, user_id: &str) -> Result<Self> {
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

    /// Returns a chat for a given chat_id or lastest one by user_id, if it exists, otherwise creates a new chat and returns it.
    /// In addition, this function uses an Arc<Mutex<Option<Chat>>> to store the chat. This is useful when you want to share the chat between multiple threads.
    pub async fn get_or_create_arc(
        app_state: Arc<AppState>,
        user_id: Arc<String>,
        chat_id: Option<Uuid>,
        chat: Arc<Mutex<Option<Chat>>>,
    ) -> Result<Arc<Self>> {
        // Lock the chat mutex
        let mut chat_lock = chat.lock().await;

        if chat_lock.is_none() {
            // Create or fetch the chat if necessary
            let new_chat = if let Some(chat_id) = chat_id {
                // Fetch by chat_id
                query_as!(
                    Chat,
                    r#"
                    SELECT * FROM chats 
                    WHERE id = $1
                    "#,
                    chat_id
                )
                .fetch_one(&app_state.pool)
                .await?
            } else {
                // Get or create by user_id
                Chat::get_or_create_by_user_id(&app_state.pool, &user_id).await?
            };
            // Assign the created chat
            *chat_lock = Some(new_chat);
        }

        // Extract and clone the chat from the Option
        match &*chat_lock {
            Some(chat) => Ok(Arc::new(chat.clone())),
            None => Err(anyhow!("Chat should be initialized but it is None")),
        }
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
