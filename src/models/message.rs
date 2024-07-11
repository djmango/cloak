use crate::models::file::{File, Filetype};
use anyhow::Result;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart,
    ChatCompletionRequestUserMessageContent, InvisibilityMetadata,
};
use chrono::{DateTime, Utc};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use sqlx::{query, FromRow, PgPool, Type};
use std::fmt;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, Type, ToSchema, PartialEq, Eq)]
#[sqlx(type_name = "role_enum", rename_all = "lowercase")] // SQL value name
#[serde(rename_all = "lowercase")] // JSON value name
pub enum Role {
    Assistant,
    System,
    Tool,
    User,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
            Role::Tool => write!(f, "tool"),
            Role::User => write!(f, "user"),
        }
    }
}

#[derive(Debug, FromRow, Serialize, Deserialize, ToSchema, Clone)]
pub struct Message {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub user_id: String,
    pub text: String,
    pub role: Role,
    pub regenerated: bool,
    pub model_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub upvoted: Option<bool>,
}

impl Default for Message {
    fn default() -> Self {
        Message {
            id: Uuid::new_v4(),
            chat_id: Uuid::nil(),
            user_id: String::new(),
            text: String::new(),
            role: Role::User,
            regenerated: false,
            model_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            upvoted: None,
        }
    }
}

impl Message {
    pub async fn new(
        pool: &PgPool,
        chat_id: Uuid,
        user_id: &str,
        model_id: Option<String>,
        text: &str,
        role: Role,
    ) -> Result<Self> {
        let message = Message {
            chat_id,
            user_id: user_id.to_string(),
            text: text.to_string(),
            role,
            model_id,
            ..Default::default()
        };

        // Save the message to the database
        query!(
            r#"
            INSERT INTO messages (id, chat_id, user_id, text, role, regenerated, model_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            message.id,
            message.chat_id,
            message.user_id,
            message.text,
            message.role.clone() as Role, // idk why this is needed but it is
            message.regenerated,
            message.model_id,
            message.created_at,
            message.updated_at
        )
        .execute(pool)
        .await?;

        Ok(message)
    }

    /// Create a new message from an OpenAI API request and saves to DB, either a user or assistant message.
    /// All other types are unsupported.
    pub async fn from_oai(
        pool: &PgPool,
        content: String,
        role: Role,
        files: Vec<String>,
        chat_id: Uuid,
        user_id: &str,
        model_id: Option<String>,
        invisibility_metadata: Option<InvisibilityMetadata>,
        created_at: Option<DateTime<Utc>>,
    ) -> Result<Self> {
     
        let message = Message {
            id: invisibility_metadata
                .as_ref()
                .map_or_else(Uuid::new_v4, |metadata| metadata.user_message_id),
            chat_id,
            user_id: user_id.to_string(),
            text: content,
            role,
            model_id,
            created_at: created_at.unwrap_or_else(Utc::now),
            ..Default::default()
        };

        // Save the message to the database
        query!(
            r#"
            INSERT INTO messages (id, chat_id, user_id, text, role, regenerated, model_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            message.id,
            message.chat_id,
            message.user_id,
            message.text,
            message.role.clone() as Role, // idk why this is needed but it is
            message.regenerated,
            message.model_id,
            message.created_at,
            message.updated_at
        )
        .execute(pool)
        .await?;

        // Join futures
        let mut file_futres = Vec::new();
        for (index, file_url) in files.iter().enumerate() {
            let file = File::new(
                chat_id,
                user_id,
                message.id,
                Filetype::Jpeg,
                // Basically, what this block is doing is checking if the file should be shown to the user or not
                // If the metadata is not present, it will default to true
                // If the metadata is present, it will check if the file should be shown to the user or not
                // If the metadata is present but out of index, it will default to true
                invisibility_metadata
                    .as_ref()
                    .and_then(|metadata| {
                        metadata
                            .show_files_to_user
                            .as_ref()
                            .and_then(|show_files| show_files.get(index))
                    })
                    .copied()
                    .unwrap_or(true),
                Some(file_url.clone()),
            );
            let file_future = query!(
                r#"
                INSERT INTO files (id, chat_id, user_id, message_id, filetype, show_to_user, url, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                file.id,
                file.chat_id,
                file.user_id,
                file.message_id,
                file.filetype.clone() as Filetype,
                file.show_to_user,
                file.url,
                file.created_at,
                file.updated_at
            )
            .execute(pool);
            file_futres.push(file_future);
        }

        join_all(file_futres).await;

        Ok(message)
    }

    /// Regenerate messages based on a given message ID.
    pub async fn mark_regenerated_from_message_id(pool: &PgPool, message_id: Uuid) -> Result<()> {
        // SQL query to update the regenerated flag
        let query_str = r#"
            WITH msg AS (
                SELECT chat_id, created_at, user_id
                FROM messages
                WHERE id = $1
            )
            UPDATE messages
            SET regenerated = true
            WHERE chat_id = (SELECT chat_id FROM msg)
              AND created_at >= (SELECT created_at FROM msg)
              AND user_id = (SELECT user_id FROM msg)
        "#;

        // Perform the query
        query(query_str)
            .bind(message_id) // Bind the message id
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn get_latest_message_by_user_id(
        pool: &PgPool,
        user_id: &str,
    ) -> Result<Option<Message>> {
        let query_str = r#"
            SELECT * FROM messages 
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT 1
        "#;

        let message = sqlx::query_as::<_, Message>(query_str)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

        Ok(message)
    }

    // Get all messages for a given user ID, optionally within a specified time range
    pub async fn get_messages_by_user_id(
        pool: &PgPool,
        user_id: &str,
        range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    ) -> Result<Vec<Message>> {
        let query_str = match range {
            Some(_) => {
                r#"
                SELECT * FROM messages 
                WHERE user_id = $1 AND created_at BETWEEN $2 AND $3
                ORDER BY created_at DESC
            "#
            }
            None => {
                r#"
                SELECT * FROM messages 
                WHERE user_id = $1
                ORDER BY created_at DESC
            "#
            }
        };

        let mut query = sqlx::query_as::<_, Message>(query_str).bind(user_id);

        if let Some((start_time, end_time)) = range {
            query = query.bind(start_time).bind(end_time);
        }

        let messages = query.fetch_all(pool).await?;

        Ok(messages)
    }

    pub async fn upvote(pool: &PgPool, message_id: Uuid, user_id: &str) -> Result<()> {
        let query_str = r#"
            UPDATE messages
            SET upvoted = true
            WHERE id = $1 AND user_id = $2
        "#;

        query(query_str)
            .bind(message_id)
            .bind(user_id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn downvote(pool: &PgPool, message_id: Uuid, user_id: &str) -> Result<()> {
        let query_str = r#"
        UPDATE messages
        SET upvoted = false
        WHERE id = $1 AND user_id = $2
        "#;

        query(query_str)
            .bind(message_id)
            .bind(user_id)
            .execute(pool)
            .await?;

        Ok(())
    }
}
