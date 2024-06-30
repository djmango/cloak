use sqlx::{query, query_as, FromRow, PgPool};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryPrompt {
    pub id: Uuid,
    pub prompt: String,
    pub example: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Default for MemoryPrompt {
    fn default() -> Self {
        MemoryPrompt {
            id: Uuid::new_v4(),
            prompt: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
            example: None,
        }
    }
}

impl MemoryPrompt {
    pub async fn new(pool: &PgPool, prompt: &str, example: Option<String>) -> Result<Self> {
        let prompt = MemoryPrompt {
            id: Uuid::new_v4(),
            prompt: prompt.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
            example: example,
        };

        // Save prompt to database
        query!(
            r#"
            INSERT INTO memory_prompts (id, prompt, created_at, updated_at, deleted_at, example) 
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            prompt.id,
            prompt.prompt,
            prompt.created_at,
            prompt.updated_at,
            prompt.deleted_at,
            prompt.example
        )
        .execute(pool)
        .await?;

        Ok(prompt)
    }

    pub async fn get_by_id(pool: &PgPool, prompt_id: Uuid) -> Result<Self> {
        let prompt = query_as!(
            MemoryPrompt,
            "SELECT * FROM memory_prompts WHERE id = $1",
            prompt_id
        )
        .fetch_one(pool)
        .await?;

        Ok(prompt)
    }

    #[allow(dead_code)]
    pub async fn get_all(pool: &PgPool) -> Result<Vec<Self>> {
        let records = query_as!(
            MemoryPrompt,
            "SELECT * FROM memory_prompts"
        )
        .fetch_all(pool)
        .await?;

        Ok(records) 
    }
}