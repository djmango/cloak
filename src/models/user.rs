use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use anyhow::Error;
use tracing::info;

use crate::routes::auth::WorkOSUser;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String, // user_01E4ZCR3C56J083X43JQXF3JK5
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: String,
    pub linked_to_keywords: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for User {
    fn default() -> Self {
        User {
            id: String::new(), // These will be ignored by the `new` constructor
            first_name: None,
            last_name: None,
            email: String::new(),
            linked_to_keywords: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl User {
    pub fn new(id: &str, first_name: &str, last_name: &str, email: &str) -> Self {
        User {
            id: id.to_string(),
            first_name: Some(first_name.to_string()),
            last_name: Some(last_name.to_string()),
            email: email.to_string(),
            ..Default::default()
        }
    }

    pub fn from_workos_user(workos_user: WorkOSUser) -> Self {
        User {
            id: workos_user.id,
            first_name: workos_user.first_name,
            last_name: workos_user.last_name,
            email: workos_user.email,
            created_at: workos_user.created_at,
            updated_at: workos_user.updated_at,
            ..Default::default()
        }
    }

    pub async fn get_or_create(
        pool: &PgPool,
        id: &str,
        first_name: &str,
        last_name: &str,
        email: &str,
    ) -> Result<Self, sqlx::Error> {
        // Attempt to find the user in the database
        if let Some(existing_user) = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?
        {
            return Ok(existing_user);
        }

        // If the user is not found, create a new entry
        let new_user = User::new(id, first_name, last_name, email);

        sqlx::query!(
            r#"
            INSERT INTO users (id, first_name, last_name, email, linked_to_keywords, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            new_user.id,
            new_user.first_name,
            new_user.last_name,
            new_user.email,
            new_user.linked_to_keywords,
            new_user.created_at,
            new_user.updated_at
        )
        .execute(pool)
        .await?;

        Ok(new_user)
    }

    pub async fn get_or_create_or_update(
        pool: &PgPool,
        workos_user: WorkOSUser
    ) -> Result<Self, Error> {
        // Attempt to find the user in the database
        if let Some(existing_user) = sqlx::query_as!(
            User,
            r#"
            SELECT * FROM users 
            WHERE id = $1
            "#,
            workos_user.id
        )
        .fetch_optional(pool)
        .await?
        {
            // If user exists, update certain fields
            let updated_user = User {
                id: existing_user.id,
                first_name: workos_user.first_name.clone(),
                last_name: workos_user.last_name.clone(),
                email: workos_user.email.clone(),
                created_at: workos_user.created_at,
                updated_at: Utc::now(),
                linked_to_keywords: existing_user.linked_to_keywords, // Use existing field value
            };

            info!("Updating user: {:?}", updated_user.email);

            sqlx::query!(
                r#"
                UPDATE users
                SET first_name = $1,
                    last_name = $2,
                    email = $3,
                    created_at = $4,
                    updated_at = $5
                WHERE id = $6
                "#,
                updated_user.first_name,
                updated_user.last_name,
                updated_user.email,
                updated_user.created_at,
                updated_user.updated_at,
                updated_user.id,
            )
            .execute(pool)
            .await?;

            return Ok(updated_user);
        }

        // If the user is not found, create a new entry
        let new_user = User::new(
            &workos_user.id,
            workos_user.first_name.as_deref().unwrap_or_default(),
            workos_user.last_name.as_deref().unwrap_or_default(),
            &workos_user.email,
        );

        info!("Creating user: {:?}", new_user.email);

        sqlx::query!(
            r#"
            INSERT INTO users (id, first_name, last_name, email, linked_to_keywords, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            new_user.id,
            new_user.first_name,
            new_user.last_name,
            new_user.email,
            new_user.linked_to_keywords,
            new_user.created_at,
            new_user.updated_at 
        )
        .execute(pool)
        .await?;

        Ok(new_user)
    }
}
