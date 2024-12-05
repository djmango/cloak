use anyhow::Result;
use chrono::{DateTime, Utc};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
pub struct Invite {
    pub id: Uuid,
    pub email: String,
    pub code: String,
    pub created_at: DateTime<Utc>,
}

impl Default for Invite {
    fn default() -> Self {
        Invite {
            id: Uuid::new_v4(),
            email: String::new(),
            code: String::new(),
            created_at: Utc::now(),
        }
    }
}

impl Invite {
    pub async fn create_invite(
        pool: &PgPool,
        email: &str,
        code: &str,
        invite_cache: &Cache<String, HashMap<Uuid, Invite>>,
    ) -> Result<Self> {
        let now_utc = Utc::now();
        let invite_id = Uuid::new_v4();

        let new_invite = Invite {
            id: invite_id,
            email: email.to_string(),
            code: code.to_string(),
            created_at: now_utc,
        };

        let invite = query_as!(
            Invite,
            r#"
            INSERT INTO invites (id, email, code, created_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            new_invite.id,
            new_invite.email,
            new_invite.code,
            new_invite.created_at
        )
        .fetch_one(pool)
        .await?;

        // Update the cache with the new invite
        debug!("Updating cache for new invite: {:?}", invite.id);
        if let Some(email_invites) = invite_cache.get(email).await {
            let mut updated_invites = email_invites.clone();
            updated_invites.insert(invite.id, invite.clone());
            invite_cache
                .insert(email.to_string(), updated_invites)
                .await;
        } else {
            let mut new_invites = HashMap::new();
            new_invites.insert(invite.id, invite.clone());
            invite_cache.insert(email.to_string(), new_invites).await;
        }

        debug!("Invite added: {:?}", invite);
        Ok(invite)
    }

    pub async fn get_invites_by_code(pool: &PgPool, code: &str) -> Result<Vec<Self>> {
        let start = Instant::now();

        let result = query_as!(
            Invite,
            r#"
                SELECT *
                FROM invites
                WHERE code = $1
                "#,
            code
        )
        .fetch_all(pool)
        .await?;

        debug!("Invites found by code: {:?}", result);
        let duration = start.elapsed();
        info!("Query execution time: {:?}", duration);
        Ok(result)
    }
}
