use anyhow::Result;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use moka::future::Cache;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryGroup {
    pub id: Uuid,
    pub name: String,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Default for MemoryGroup {
    fn default() -> Self {
        MemoryGroup {
            id: Uuid::new_v4(),
            name: String::new(),
            emoji: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }
}

impl MemoryGroup {
    pub fn new(
        id: Uuid, 
        name: &str, 
        emoji: &str, 
        created_at: Option<DateTime<Utc>>
    ) -> Self {
        let now = created_at.unwrap_or_else(Utc::now);
        MemoryGroup {
            id,
            name: name.to_string(),
            emoji: emoji.to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    pub async fn add_memory_group(
        pool: &PgPool,
        name: &str,
        emoji: &str,
        memory_groups_cache: &Cache<String, MemoryGroup>
    ) -> Result<Self> {
        // Check if the group is already in the cache
        if let Some(cached_group) = memory_groups_cache.get(name).await {
            debug!("Memory group found in cache: {:?}", cached_group.id);
            return Ok(cached_group);
        }

        let now_utc = Utc::now();
        let group_id = Uuid::new_v4();

        let new_group = MemoryGroup::new(
            group_id,
            name,
            emoji,
            Some(now_utc),
        );

        let group = query_as!(
            MemoryGroup,
            "INSERT INTO memory_groups (id, name, emoji, created_at, updated_at) VALUES ($1, $2, $3, $4, $5) RETURNING *",
            new_group.id,
            new_group.name,
            new_group.emoji,
            new_group.created_at,
            new_group.updated_at
        )
        .fetch_one(pool)
        .await?;

        // Update the cache with the new memory group
        debug!("Updating cache for new memory group: {:?}", group.id);
        memory_groups_cache.insert(group.name.clone(), group.clone()).await;

        debug!("Memory group added: {:?}", group);
        Ok(group)
    }

    pub async fn delete_memory_group(
        pool: &PgPool,
        group_id: Uuid,
        memory_groups_cache: &Cache<String, MemoryGroup>,
    ) -> Result<Self> {
        let group = query_as!(
            MemoryGroup,
            r#"
            UPDATE memory_groups 
            SET deleted_at = $1
            WHERE id = $2 AND deleted_at IS NULL
            RETURNING *
            "#,
            Utc::now(),
            group_id
        )
        .fetch_one(pool)
        .await?;

        // Remove the deleted group from the cache
        memory_groups_cache.remove(&group.name).await;
        info!(
            "Removed memory group {} from cache",
            group_id
        );

        debug!("Memory group soft-deleted with id: {:?}", group_id);
        Ok(group)
    }

    pub async fn update_memory_group(
        pool: &PgPool,
        group_id: Uuid,
        new_name: &str,
        new_emoji: &str,
        memory_groups_cache: &Cache<String, MemoryGroup>,
    ) -> Result<Self> {
        let now_utc = Utc::now();

        let group = query_as!(
            MemoryGroup,
            r#"
            SELECT *
            FROM memory_groups 
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            group_id
        )
        .fetch_one(pool)
        .await?;

        let updated_group = MemoryGroup {
            name: new_name.to_string(),
            emoji: new_emoji.to_string(),
            updated_at: now_utc,
            ..group
        };

        let group = query_as!(
            MemoryGroup,
            r#"
            UPDATE memory_groups 
            SET name = $1, emoji = $2, updated_at = $3
            WHERE id = $4 AND deleted_at IS NULL
            RETURNING *
            "#,
            updated_group.name,
            updated_group.emoji,
            updated_group.updated_at,
            updated_group.id
        )
        .fetch_one(pool)
        .await?;

        info!("Updating cache for memory group: {:?}", group_id);
        // Remove the old entry and add the updated one
        memory_groups_cache.remove(&group.name).await;
        memory_groups_cache.insert(group.name.clone(), group.clone()).await;

        debug!("Memory group updated: {:?}", group);
        Ok(group)
    }

    pub async fn get_memory_group(
        pool: &PgPool,
        group_id: Uuid,
        memory_groups_cache: &Cache<String, MemoryGroup>,
    ) -> Result<Self> {
        // Try to get from cache first
        if let Some(cached_group) = memory_groups_cache.iter().find(|(_, v)| v.id == group_id) {
            debug!("Memory group found in cache: {:?}", group_id);
            return Ok(cached_group.1.clone());
        }

        // If not in cache, get from database
        let group = query_as!(
            MemoryGroup,
            r#"
            SELECT *
            FROM memory_groups 
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            group_id
        )
        .fetch_one(pool)
        .await?;

        // Update cache with fetched group
        memory_groups_cache.insert(group.name.clone(), group.clone()).await;
        debug!("Memory group added to cache: {:?}", group_id);

        Ok(group)
    }
}