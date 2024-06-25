// models/memory.rs

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Memory {
    pub id: Uuid,
    pub user_id: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Default for Memory {
    fn default() -> Self {
        Memory {
            id: Uuid::new_v4(),
            user_id: String::new(),
            content: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }
}

impl Memory {
    pub async fn add_memory(pool: &PgPool, memory: &str, user_id: &str) -> Result<Self> {
        let now_utc = Utc::now();
        let memory_id = Uuid::new_v4();

        let new_memory = Memory::new(memory_id, user_id, memory, Some(now_utc));

        let memory = query_as!(
            Memory,
            "INSERT INTO memories (id, user_id, created_at, updated_at, content) VALUES ($1, $2, $3, $4, $5) RETURNING *",
            new_memory.id,
            new_memory.user_id,
            new_memory.created_at,
            new_memory.updated_at,
            new_memory.content
        )
        .fetch_one(pool)
        .await?;

        debug!("Memory added: {:?}", memory);
        Ok(memory)
    }

    pub async fn update_memory(
        pool: &PgPool,
        memory_id: Uuid,
        new_memory: &str,
        user_id: &str,
    ) -> Result<Self> {
        let now_utc = Utc::now();

        let memory = query_as!(
            Memory,
            r#"
            SELECT id, user_id, created_at, updated_at, content, deleted_at 
            FROM memories 
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
            "#,
            memory_id,
            user_id
        )
        .fetch_one(pool)
        .await?;

        let updated_memory = Memory {
            content: new_memory.to_string(),
            updated_at: now_utc,
            ..memory
        };

        let memory = query_as!(
            Memory,
            r#"
            UPDATE memories 
            SET content = $1, updated_at = $2
            WHERE id = $3 AND user_id = $4 AND deleted_at IS NULL
            RETURNING *
            "#,
            updated_memory.content,
            updated_memory.updated_at,
            updated_memory.id,
            updated_memory.user_id
        )
        .fetch_one(pool)
        .await?;

        debug!("Memory updated: {:?}", memory);
        Ok(memory)
    }

    pub async fn get_all_memories(pool: &PgPool, user_id: &str) -> Result<Vec<Self>> {
        let result = query_as!(
            Memory,
            r#"
            SELECT id, user_id, created_at, updated_at, content, deleted_at 
            FROM memories 
            WHERE user_id = $1 AND deleted_at IS NULL
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        debug!("All memories found: {:?}", result);
        Ok(result)
    }

    pub async fn delete_memory(pool: &PgPool, memory_id: Uuid, user_id: &str) -> Result<()> {
        query!(
            r#"
            UPDATE memories 
            SET deleted_at = $1
            WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL
            "#,
            Utc::now(),
            memory_id,
            user_id
        )
        .execute(pool)
        .await?;

        debug!("Memory soft-deleted with id: {:?}", memory_id);
        Ok(())
    }

    pub fn format_memories(memories: Vec<Self>) -> String {
        let formatted_memories: Vec<String> = memories
            .iter()
            .map(|memory| {
                let timestamp = memory.created_at.with_timezone(&Utc);
                format!(
                    "{}: [{}] {};",
                    memory.id,
                    timestamp.format("%m/%d/%y %I:%M %p %Z"),
                    memory.content
                )
            })
            .collect();

        formatted_memories.join("\n")
    }
}

impl Memory {
    pub fn new(id: Uuid, user_id: &str, content: &str, created_at: Option<DateTime<Utc>>) -> Self {
        Memory {
            id,
            user_id: user_id.to_string(),
            created_at: created_at.unwrap_or_else(Utc::now),
            updated_at: Utc::now(),
            content: content.to_string(),
            ..Default::default()
        }
    }
}
