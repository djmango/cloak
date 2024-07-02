// models/memory.rs

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};
use tracing::{debug, info, error};
use uuid::Uuid;
use regex::Regex;
use std::time::Instant;
use lazy_static::lazy_static;
use moka::future::Cache;
use std::collections::HashMap;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Memory {
    pub id: Uuid,
    pub user_id: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub memory_prompt_id: Option<Uuid>,
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
            memory_prompt_id: None,
        }
    }
}

lazy_static! {
    static ref USER_INFO_REGEX: Regex = Regex::new(r"(?s)<user information>(.*?)</user information>").unwrap();
}

impl Memory {
    pub async fn add_memory(pool: &PgPool, memory: &str, user_id: &str, prompt_id: Option<Uuid>, memory_cache: &Cache<String, HashMap<Uuid, Memory>>) -> Result<Self> {
        let now_utc = Utc::now();
        let memory_id = Uuid::new_v4();

        let new_memory = Memory::new(memory_id, user_id, memory, prompt_id, Some(now_utc));

        let memory = query_as!(
            Memory,
            "INSERT INTO memories (id, user_id, created_at, updated_at, content, memory_prompt_id) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
            new_memory.id,
            new_memory.user_id,
            new_memory.created_at,
            new_memory.updated_at,
            new_memory.content,
            new_memory.memory_prompt_id
        )
        .fetch_one(pool)
        .await?;

        // Update the cache with the new memory
        info!("Updating cache for new memory: {:?}", memory.id);
        if let Some(user_memories) = memory_cache.get(user_id).await {
            let mut updated_user_memories = user_memories.clone();
            updated_user_memories.insert(memory.id, memory.clone());
            memory_cache.insert(user_id.to_string(), updated_user_memories).await;
        } else {
            let mut new_user_memories = HashMap::new();
            new_user_memories.insert(memory.id, memory.clone());
            memory_cache.insert(user_id.to_string(), new_user_memories).await;
        }

        debug!("Memory added: {:?}", memory);
        Ok(memory)
    }

    pub async fn update_memory(
        pool: &PgPool,
        memory_id: Uuid,
        new_memory: &str,
        user_id: &str,
        memory_cache: &Cache<String, HashMap<Uuid, Memory>>
    ) -> Result<Self> {
        let now_utc = Utc::now();

        let memory = query_as!(
            Memory,
            r#"
            SELECT id, user_id, created_at, updated_at, content, deleted_at, memory_prompt_id
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

        info!("Updating cache for memory: {:?}", memory_id);
        if let Some(user_memories) = memory_cache.get(user_id).await {
            let mut updated_user_memories = user_memories.clone();
            updated_user_memories.insert(memory.id, memory.clone());
            memory_cache.insert(user_id.to_string(), updated_user_memories).await;
        } else {
            let mut new_user_memories = HashMap::new();
            new_user_memories.insert(memory.id, memory.clone());
            memory_cache.insert(user_id.to_string(), new_user_memories).await;
        }

        debug!("Memory updated: {:?}", memory);
        Ok(memory)
    }

    pub async fn delete_all_memories(
        pool: &PgPool,
        user_id: &str,
        memory_cache: &Cache<String, HashMap<Uuid, Memory>>
    ) -> Result<Vec<Uuid>> {
        let deleted_memories = sqlx::query!(
            "UPDATE memories 
            SET deleted_at = $1
            WHERE user_id = $2 AND deleted_at IS NULL
            RETURNING id",
            Utc::now(),
            user_id
        )
        .fetch_all(pool)
        .await?;

        let deleted_ids: Vec<Uuid> = deleted_memories.into_iter().map(|row| row.id).collect();

        if let Some(mut user_memories) = memory_cache.get(user_id).await {
            for id in &deleted_ids {
                user_memories.remove(id);
            }
            memory_cache.insert(user_id.to_string(), user_memories).await;
            info!("Removed {} deleted memories from cache", deleted_ids.len());
        }

        info!("All memories soft-deleted for user: {}. Affected rows: {}", user_id, deleted_ids.len());
        Ok(deleted_ids)
    }
    
    pub async fn delete_memory(pool: &PgPool, memory_id: Uuid, user_id: &str, memory_cache: &Cache<String, HashMap<Uuid, Memory>>) -> Result<Memory> {
        let memory = query_as!(
            Memory,
            r#"
            UPDATE memories 
            SET deleted_at = $1
            WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL
            RETURNING *
            "#,
            Utc::now(),
            memory_id,
            user_id
        )
        .fetch_one(pool)
        .await?;

        if let Some(mut user_memories) = memory_cache.get(user_id).await {
            user_memories.remove(&memory_id);
            memory_cache.insert(user_id.to_string(), user_memories).await;
            info!("Removed memory {} from cache for user {}", memory_id, user_id);
        }

        debug!("Memory soft-deleted with id: {:?}", memory_id);
        Ok(memory)
    }

    pub async fn get_all_memories(pool: &PgPool, user_id: &str, memory_prompt_id: Option<Uuid>, memory_cache: &Cache<String, HashMap<Uuid, Memory>>) -> Result<Vec<Self>> {
        let start = Instant::now();
        
        // Try to get memories from cache first
        if let Some(user_memories) = memory_cache.get(user_id).await {
            let cached_memories: Vec<Self> = user_memories.values().cloned().collect();
            let filtered_memories = match memory_prompt_id {
                Some(prompt_id) => cached_memories.into_iter()
                    .filter(|memory| memory.memory_prompt_id == Some(prompt_id))
                    .collect(),
                None => cached_memories,
            };
            
            let duration = start.elapsed();
            info!("Query execution time: {:?}", duration);
            info!("Retrieved {} memories from cache for user: {}", filtered_memories.len(), user_id);
            return Ok(filtered_memories);
        }

        // If not in cache, fetch from database
        let result = query_as!(
            Memory,
            r#"
            SELECT id, user_id, created_at, updated_at, content, deleted_at, memory_prompt_id
            FROM memories 
            WHERE user_id = $1 AND deleted_at IS NULL
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;
        
        // Update cache with fetched memories
        let mut memory_map = HashMap::new();
        for memory in &result {
            memory_map.insert(memory.id, memory.clone());
        }
        memory_cache.insert(user_id.to_string(), memory_map).await;
        
        debug!("All memories found: {:?}", result);
        let duration = start.elapsed();
        info!("Query execution time: {:?}", duration);
        Ok(result)
    }

    pub fn format_memories(memories: Vec<Self>) -> String {
        let mut formatted_memories = String::new();

        for (index, memory) in memories.iter().enumerate() {
            info!("Memory {}: Content: {:?}", index, memory.content);
            if let Some(captures) = USER_INFO_REGEX.captures(&memory.content) {
                info!("Memory {}: Regex match found", index);
                if let Some(content) = captures.get(1) {
                    let extracted = content.as_str().trim();
                    info!("Memory {}: Extracted content length: {} chars", index, extracted.len());
                    formatted_memories.push_str(extracted);
                    formatted_memories.push_str("\n");
                }
            } else {
                info!("Memory {}: No regex match found", index);
            }
        }
        formatted_memories.trim_end().to_string()
    }
}

impl Memory {
    pub fn new(id: Uuid, user_id: &str, content: &str, prompt_id: Option<Uuid>, created_at: Option<DateTime<Utc>>) -> Self {
        Memory {
            id,
            user_id: user_id.to_string(),
            created_at: created_at.unwrap_or_else(Utc::now),
            updated_at: Utc::now(),
            content: content.to_string(),
            memory_prompt_id: prompt_id,
            ..Default::default()
        }
    }
}
