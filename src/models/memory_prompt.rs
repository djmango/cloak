use sqlx::{query, query_as, FromRow, PgPool};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryPrompt {
    pub id: Uuid,
    pub prompt: String,
    pub upvotes: i32,
    pub created_at: DateTime<Utc>,
    pub example: Option<String>,
}

impl Default for MemoryPrompt {
    fn default() -> Self {
        MemoryPrompt {
            id: Uuid::new_v4(),
            prompt: String::new(),
            upvotes: 0,
            created_at: Utc::now(),
            example: None,
        }
    }
}

impl MemoryPrompt {
    pub async fn new(pool: &PgPool, prompt: &str, example: Option<String>) -> Result<Self> {
        let prompt = MemoryPrompt {
            id: Uuid::new_v4(),
            prompt: prompt.to_string(),
            upvotes: 0,
            created_at: Utc::now(),
            example: example,
        };

        // Save prompt to database
        query!(
            r#"
            INSERT INTO memory_prompts (id, prompt, upvotes, created_at, example) 
            VALUES ($1, $2, $3, $4, $5)
            "#,
            prompt.id,
            prompt.prompt,
            prompt.upvotes,
            prompt.created_at,
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
    pub async fn upvote(pool: &PgPool, prompt_id: Uuid) -> Result<()> {
        query!("UPDATE memory_prompts SET upvotes = upvotes + 1 WHERE id = $1", prompt_id)
            .execute(pool)
            .await?;
        Ok(())
    }
    
    #[allow(dead_code)]
    pub async fn downvote(pool: &PgPool, prompt_id: Uuid) -> Result<()> {
        query!("UPDATE memory_prompts SET upvotes = upvotes - 1 WHERE id = $1", prompt_id)
            .execute(pool)
            .await?;
        Ok(())
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