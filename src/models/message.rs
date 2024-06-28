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
use utoipa::ToSchema;
use uuid::Uuid;
use std::fmt;

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
    pub memory_ids: Option<Vec<Uuid>>,
    pub upvoted: Option<bool>,
    pub memory_prompt_id: Option<Uuid>,
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
            memory_ids: None,
            upvoted: None,
            memory_prompt_id: None,
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
        memory_ids: Option<Vec<Uuid>>,
        memory_prompt_id: Option<Uuid>,
    ) -> Result<Self> {
        let message = Message {
            chat_id,
            user_id: user_id.to_string(),
            text: text.to_string(),
            role,
            model_id,
            memory_ids,
            memory_prompt_id,
            ..Default::default()
        };

        // Save the message to the database
        query!(
            r#"
            INSERT INTO messages (id, chat_id, user_id, text, role, regenerated, model_id, memory_ids, created_at, updated_at, memory_prompt_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            message.id,
            message.chat_id,
            message.user_id,
            message.text,
            message.role.clone() as Role, // idk why this is needed but it is
            message.regenerated,
            message.model_id,
            message.memory_ids.as_deref(),
            message.created_at,
            message.updated_at,
            message.memory_prompt_id
        )
        .execute(pool)
        .await?;

        Ok(message)
    }

    /// Create a new message from an OpenAI API request and saves to DB, either a user or assistant message.
    /// All other types are unsupported.
    pub async fn from_oai(
        pool: &PgPool,
        oai_message: ChatCompletionRequestMessage,
        chat_id: Uuid,
        user_id: &str,
        model_id: Option<String>,
        invisibility_metadata: Option<InvisibilityMetadata>,
        created_at: Option<DateTime<Utc>>,
    ) -> Result<Self> {
        // Determine the message content and role, and always ensure at least a blank string unless
        // its an unhandled message type
        let (content, role, files) = match oai_message {
            ChatCompletionRequestMessage::User(user_message) => match user_message.content {
                ChatCompletionRequestUserMessageContent::Text(text) => (text, Role::User, vec![]),
                ChatCompletionRequestUserMessageContent::Array(array) => {
                    let mut concatenated_text = String::new();
                    let mut file_urls = Vec::new();

                    for part in &array {
                        match part {
                            ChatCompletionRequestMessageContentPart::Text(text_part) => {
                                if !text_part.text.trim().is_empty() {
                                    concatenated_text.push_str(&text_part.text);
                                }
                            }
                            ChatCompletionRequestMessageContentPart::ImageUrl(image_part) => {
                                file_urls.push(image_part.image_url.url.clone());
                            }
                        }
                    }
                    (concatenated_text, Role::User, file_urls)
                }
            },
            ChatCompletionRequestMessage::Assistant(assistant_message) => {
                if let Some(content) = &assistant_message.content {
                    (content.clone(), Role::Assistant, vec![])
                } else {
                    ("".to_string(), Role::Assistant, vec![])
                }
            }
            _ => return Err(anyhow::anyhow!("Unsupported message type")),
        };

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
            memory_ids: None,
            ..Default::default()
        };

        // Save the message to the database
        query!(
            r#"
            INSERT INTO messages (id, chat_id, user_id, text, role, regenerated, model_id, memory_ids, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            message.id,
            message.chat_id,
            message.user_id,
            message.text,
            message.role.clone() as Role, // idk why this is needed but it is
            message.regenerated,
            message.model_id,
            message.memory_ids.as_deref(),
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

    pub async fn get_by_id(pool: &PgPool, message_id: Uuid) -> Result<Message> {
        let query_str = r#"
            SELECT * FROM messages WHERE id = $1
        "#;

        let row = query(query_str)
            .bind(message_id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Message::from_row(&row)?),
            None => Err(anyhow::anyhow!("Message not found")),
        }
    }

    // Get all messages for a given user ID
    #[allow(dead_code)]
    pub async fn get_messages_by_user_id(pool: &PgPool, user_id: &str) -> Result<Vec<Message>> {
        let query_str = r#"
            SELECT * FROM messages WHERE user_id = $1'
        "#;

        let rows = query(query_str)
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let messages = rows.into_iter().map(|row| Message::from_row(&row).unwrap()).collect::<Vec<Message>>();

        Ok(messages)
    }

    // Get all messages for a given chat ID
    pub async fn get_messages_by_chat_id(pool: &PgPool, chat_id: Uuid) -> Result<Vec<Message>> {
        let query_str = r#"
            SELECT * FROM messages WHERE chat_id = $1 ORDER BY created_at ASC
        "#;

        let rows = query(query_str)
            .bind(chat_id)
            .fetch_all(pool)
            .await?;

        let messages = rows.into_iter().map(|row| Message::from_row(&row).unwrap()).collect::<Vec<Message>>();

        Ok(messages)
    }

    pub async fn get_next_msg(pool: &PgPool, chat_id: Uuid, last_msg: &Message) -> Result<Option<Message>> {
        let query_str = r#"
            SELECT * FROM messages 
            WHERE chat_id = $1 
              AND created_at > $2 
              AND role = 'user'
            ORDER BY created_at ASC 
            LIMIT 1
        "#;

        let row = query(query_str)
            .bind(chat_id)
            .bind(last_msg.created_at)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(Message::from_row(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn upvote (pool: &PgPool, message_id: Uuid, user_id: &str) ->  Result<()> {
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

    pub async fn downvote (pool: &PgPool, message_id: Uuid, user_id: &str) ->  Result<()> {
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
