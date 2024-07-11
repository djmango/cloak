use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
pub struct Recording {
    pub id: Uuid,
    pub session_id: Uuid,
    pub s3_object_key: String,
    pub start_timestamp: chrono::NaiveDateTime,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Default for Recording {
    fn default() -> Self {
        Recording {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            s3_object_key: String::new(),
            start_timestamp: chrono::NaiveDateTime::default(),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Recording {
    pub async fn new(
        pool: &PgPool,
        session_id: Uuid,
        s3_object_key: String,
        start_timestamp: i64,
    ) -> Result<Self> {
        let start_timestamp = chrono::DateTime::from_timestamp(start_timestamp, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid start_timestamp"))?
            .naive_utc();

        let recording = Recording {
            id: Uuid::new_v4(),
            session_id,
            s3_object_key,
            start_timestamp,
            ..Default::default()
        };

        query!(
            r#"
            INSERT INTO recordings (id, session_id, s3_object_key, start_timestamp, created_at, updated_at) 
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            recording.id, recording.session_id, recording.s3_object_key, recording.start_timestamp, recording.created_at, recording.updated_at
        )
        .execute(pool)
        .await?;

        Ok(recording)
    }
}