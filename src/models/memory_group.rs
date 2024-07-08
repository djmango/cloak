use anyhow::Result;
use chrono::{DateTime, Utc};
use moka::future::Cache;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryGroup {
    pub id: Uuid,
    pub user_id: String,
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
            user_id: String::new(),
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
        user_id: &str,
        name: &str, 
        emoji: &str, 
        created_at: Option<DateTime<Utc>>
    ) -> Self {
        let now = created_at.unwrap_or_else(Utc::now);
        MemoryGroup {
            id,
            user_id: user_id.to_string(),
            name: name.to_string(),
            emoji: emoji.to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    pub async fn add_memory_group(
        pool: &PgPool,
        user_id: &str,
        name: &str,
        emoji: &str,
        memory_groups_cache: &Cache<String, HashMap<String, MemoryGroup>>
    ) -> Result<Self> {
        // Check if the group is already in the cache
        if let Some(user_memory_groups) = memory_groups_cache.get(user_id).await {
            if let Some(cached_group) = user_memory_groups.get(name) {
                debug!("Memory group found in cache: {:?}", cached_group.id);
                return Ok(cached_group.clone());
            }
        }

        let now_utc = Utc::now();
        let group_id = Uuid::new_v4();

        let new_group = MemoryGroup::new(
            group_id,
            user_id,
            name,
            emoji,
            Some(now_utc),
        );

        let group = query_as!(
            MemoryGroup,
            r#"
            INSERT INTO memory_groups (
                id, 
                user_id, 
                name, 
                emoji, 
                created_at, 
                updated_at
            ) 
            VALUES ($1, $2, $3, $4, $5, $6) 
            RETURNING *
            "#,
            new_group.id,
            new_group.user_id,
            new_group.name,
            new_group.emoji,
            new_group.created_at,
            new_group.updated_at
        )
        .fetch_one(pool)
        .await?;

        // Update the cache with the new memory group
        info!("Updating cache for new memory group: {:?}", group.id);
        match memory_groups_cache.get(&group.user_id).await {
            Some(user_memory_groups) => {
                let mut updated_user_memory_groups = user_memory_groups.clone();
                updated_user_memory_groups.insert(group.name.clone(), group.clone());
                memory_groups_cache
                    .insert(group.user_id.clone(), updated_user_memory_groups)
                    .await;
                info!(
                    "Added memory group {} to cache for user {}",
                    group.id, group.user_id
                );
            },
            None => {
                let mut new_user_groups = HashMap::new();
                new_user_groups.insert(group.name.clone(), group.clone());
                memory_groups_cache
                    .insert(group.user_id.clone(), new_user_groups)
                    .await;
                info!(
                    "Created new cache entry for user {} with group {}",
                    group.user_id, group.id
                );
            }
        }

        debug!("Memory group added: {:?}", group);
        Ok(group)
    }

    #[allow(dead_code)]
    pub async fn delete_memory_group(
        pool: &PgPool,
        group_id: Uuid,
        memory_groups_cache: &Cache<String, HashMap<String, MemoryGroup>>,
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
        info!("Updating cache for deleted memory group: {:?}", group.id);
        match memory_groups_cache.get(&group.user_id).await {
            Some(user_memory_groups) => {
                let mut updated_user_memory_groups = user_memory_groups.clone();
                updated_user_memory_groups.remove(&group.name);
                memory_groups_cache
                    .insert(group.user_id.clone(), updated_user_memory_groups)
                    .await;
                info!(
                    "Removed memory group {} from cache for user {}",
                    group.id, group.user_id
                );
            },
            None => {
                info!(
                    "No cache entry found for user {} when deleting group {}",
                    group.user_id, group.id
                );
            }
        }

        debug!("Memory group soft-deleted with id: {:?}", group_id);
        Ok(group)
    }

    #[allow(dead_code)]
    pub async fn update_memory_group(
        pool: &PgPool,
        group_id: Uuid,
        new_name: &str,
        new_emoji: &str,
        memory_groups_cache: &Cache<String, HashMap<String, MemoryGroup>>,
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

        // Update the cache for the updated memory group
        info!("Updating cache for updated memory group: {:?}", group.id);
        match memory_groups_cache.get(&group.user_id).await {
            Some(user_memory_groups) => {
                let mut updated_user_memory_groups = user_memory_groups.clone();
                updated_user_memory_groups.remove(&group.name);
                updated_user_memory_groups.insert(group.name.clone(), group.clone());
                memory_groups_cache
                    .insert(group.user_id.to_string(), updated_user_memory_groups)
                    .await;
                info!(
                    "Updated memory group {} in cache for user {}",
                    group.id, group.user_id
                );
            },
            None => {
                let mut new_user_groups = HashMap::new();
                new_user_groups.insert(group.name.clone(), group.clone());
                memory_groups_cache.insert(group.user_id.clone(), new_user_groups).await;
                info!(
                    "Created new cache entry for user {} with updated group {}",
                    group.user_id, group.id
                );
            }
        }

        debug!("Memory group updated: {:?}", group);
        Ok(group)
    }

    pub async fn get_memory_group(
        pool: &PgPool,
        user_id: &str,
        grouping: &str,
        memory_groups_cache: &Cache<String, HashMap<String, MemoryGroup>>,
    ) -> Result<Option<Self>> {
        // Try to get from cache first using user_id as key
        if let Some(user_groups) = memory_groups_cache.get(user_id).await {
            if let Some(cached_group) = user_groups.get(grouping) {
                debug!("Memory group found in cache: {:?}", grouping);
                return Ok(Some(cached_group.clone()));
            }
        }

        // If not in cache, get from database
        let group = query_as!(
            MemoryGroup,
            r#"
            SELECT *
            FROM memory_groups 
            WHERE user_id = $1 AND name = $2 AND deleted_at IS NULL
            LIMIT 1
            "#,
            user_id,
            grouping
        )
        .fetch_optional(pool)
        .await?;

        match group {
            Some(group) => {
                info!("Updating cache for memory group: {:?}", group.id);
                match memory_groups_cache.get(&group.user_id).await {
                    Some(user_memory_groups) => {
                        let mut updated_user_memory_groups = user_memory_groups.clone();
                        updated_user_memory_groups.remove(&group.name);
                        updated_user_memory_groups.insert(group.name.clone(), group.clone());
                        memory_groups_cache
                            .insert(group.user_id.to_string(), updated_user_memory_groups)
                            .await;
                        info!(
                            "Updated memory group {} in cache for user {}",
                            group.id, group.user_id
                        );
                    },
                    None => {
                        let mut new_user_groups = HashMap::new();
                        new_user_groups.insert(group.name.clone(), group.clone());
                        memory_groups_cache.insert(group.user_id.clone(), new_user_groups).await;
                        info!(
                            "Created new cache entry for user {} with group {}",
                            group.user_id, group.id
                        );
                    }
                }
                Ok(Some(group))
            },
            None => Ok(None)
        }
    }
}