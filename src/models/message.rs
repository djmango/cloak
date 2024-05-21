use anyhow::Error;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart,
    ChatCompletionRequestUserMessageContent,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, FromRow, PgPool};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "role_enum", rename_all = "lowercase")] // SQL value name
pub enum Role {
    Assistant,
    System,
    Tool,
    User,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
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

    /// Create a new message from an OpenAI API request and saves to DB, either a user or assistant message.
    /// All other types are unsupported.
    pub async fn from_oai(
        pool: &PgPool,
        oai_message: ChatCompletionRequestMessage,
        chat_id: Uuid,
        user_id: &str,
    ) -> Result<Self, Error> {
        // Determine the message content and role, and always ensure at least a blank string unless
        // its an unhandled message type
        let (content, role) = match oai_message {
            ChatCompletionRequestMessage::User(user_message) => {
                match user_message.content {
                    ChatCompletionRequestUserMessageContent::Text(text) => (text, Role::User),
                    ChatCompletionRequestUserMessageContent::Array(array) => {
                        let mut concatenated_text = String::new();
                        for part in &array {
                            match part {
                                ChatCompletionRequestMessageContentPart::Text(text_part) => {
                                    if !text_part.text.trim().is_empty() {
                                        concatenated_text.push_str(&text_part.text);
                                        concatenated_text.push(' '); // Optional: Add a space between parts
                                    }
                                }
                                _ => {
                                    warn!("Non-text part type found: {:?}", part);
                                }
                            }
                        }
                        (concatenated_text, Role::User)
                    }
                }
            }
            ChatCompletionRequestMessage::Assistant(assistant_message) => {
                if let Some(content) = &assistant_message.content {
                    (content.clone(), Role::Assistant)
                } else {
                    ("".to_string(), Role::Assistant)
                }
            }
            _ => return Err(anyhow::anyhow!("Unsupported message type")),
        };

        let message = Message {
            chat_id,
            user_id: user_id.to_string(),
            text: content,
            role,
            ..Default::default()
        };

        // Save the message to the database
        query!(
            r#"
            INSERT INTO messages (id, chat_id, user_id, text, role, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            message.id,
            message.chat_id,
            message.user_id,
            message.text,
            message.role.clone() as Role, // idk why this is needed but it is
            message.created_at,
            message.updated_at
        )
        .execute(pool)
        .await?;

        Ok(message)
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
