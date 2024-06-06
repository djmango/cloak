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
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            id: Uuid::new_v4(),
            user_id: String::new(),
            name: String::new(),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Chat {
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

            // If chat_id is provided but not found, create a new chat with the provided chat_id and user_id
            if let Some(chat) = query_as!(
                Chat,
                r#"
                INSERT INTO chats (id, user_id, name, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING *
                "#,
                chat_id,
                user_id,
                "New Chat",
                Utc::now(),
                Utc::now()
            )
            .fetch_optional(pool)
            .await?
            {
                debug!("Chat created: {:?}", chat);
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
        let chat = Chat {
            user_id: user_id.to_string(),
            name: "New Chat".to_string(),
            ..Default::default()
        };
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

    /// Updates the name of the chat
    pub async fn update_name(
        pool: &PgPool,
        chat_id: Uuid,
        user_id: &str,
        new_name: &str,
    ) -> Result<Self> {
        let chat = query_as!(
            Chat,
            r#"
            UPDATE chats
            SET name = $1, updated_at = $2
            WHERE id = $3 AND user_id = $4
            RETURNING *
            "#,
            new_name,
            Utc::now(),
            chat_id,
            user_id
        )
        .fetch_one(pool)
        .await?;

        debug!("Chat updated: {:?}", chat);
        Ok(chat)
    }

    /// Soft deletes a chat by chat_id and user_id
    pub async fn delete(pool: &PgPool, chat_id: Uuid, user_id: &str) -> Result<()> {
        query_as!(
            Chat,
            r#"
            UPDATE chats
            SET deleted_at = $1
            WHERE id = $2 AND user_id = $3
            "#,
            Utc::now(),
            chat_id,
            user_id
        )
        .execute(pool)
        .await?;

        debug!("Chat soft-deleted with id: {:?}", chat_id);
        Ok(())
    }
}
