use anyhow::Error;
// use anyhow::{anyhow, Context, Result};
use anyhow::Result;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart,
    ChatCompletionRequestUserMessageContent,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, FromRow, PgPool, Type};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
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
    pub async fn new(
        pool: &PgPool,
        chat_id: Uuid,
        user_id: &str,
        text: &str,
        role: Role,
    ) -> Result<Self, Error> {
        let message = Message {
            chat_id,
            user_id: user_id.to_string(),
            text: text.to_string(),
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

    /// Create a new message from an OpenAI API request and saves to DB, either a user or assistant message.
    /// All other types are unsupported.
    pub async fn from_oai(
        pool: &PgPool,
        oai_message: ChatCompletionRequestMessage,
        chat_id: Uuid,
        user_id: &str,
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
                            ChatCompletionRequestMessageContentPart::Image(image_part) => {
                                // let image_data =
                                //     if image_part.image_url.url.starts_with("data:image/") {
                                //         let base64_data = image_part
                                //             .image_url
                                //             .url
                                //             .split(',')
                                //             .nth(1)
                                //             .context("Invalid base64 data")?;
                                //         general_purpose::STANDARD
                                //             .decode(base64_data)
                                //             .context("Failed to decode base64 data")?
                                //     } else {
                                //         let response =
                                //             reqwest::get(&image_part.image_url.url).await?;
                                //         response.bytes().await?.to_vec()
                                //     };

                                // let file_name = format!("{}.png", Uuid::new_v4());
                                // let file_url =
                                //     upload_to_cloudflare(&image_data, &file_name).await?;
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
            chat_id,
            user_id: user_id.to_string(),
            text: content,
            role,
            files: if files.is_empty() { None } else { Some(files) },
            ..Default::default()
        };

        // Save the message to the database
        query!(
            r#"
            INSERT INTO messages (id, chat_id, user_id, text, role, files, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            message.id,
            message.chat_id,
            message.user_id,
            message.text,
            message.role.clone() as Role, // idk why this is needed but it is
            message.files.as_deref(),
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

// async fn upload_to_cloudflare(image_data: &[u8], file_name: &str) -> Result<String> {
//     let client = Client::new();
//     let url = format!("https://your-cloudflare-r2-endpoint/{}", file_name);

//     let response = client.put(&url).body(image_data.to_vec()).send().await?;

//     if response.status().is_success() {
//         Ok(url)
//     } else {
//         Err(anyhow!("Failed to upload image to Cloudflare"))
//     }
// }
