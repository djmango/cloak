use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool };
use anyhow::Error;
use indicatif::ProgressIterator;

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
    pub fn new(id: &str, first_name: &str, last_name: &str, email: &str, created_at: Option<DateTime<Utc>>) -> Self {
        User {
            id: id.to_string(),
            first_name: Some(first_name.to_string()),
            last_name: Some(last_name.to_string()),
            email: email.to_string(),
            created_at: created_at.unwrap_or_else(Utc::now),
            ..Default::default()
        }
    }

    pub async fn get_or_create_or_update_bulk_workos(
        pool: &PgPool,
        workos_users: Vec<WorkOSUser>,
    ) -> Result<Vec<Self>, Error> {
        let mut transaction = pool.begin().await?;

        let mut user_results = Vec::new();

        for workos_user in workos_users.into_iter().progress() {
            if let Some(existing_user) = query_as!(
                User,
                r#"
                SELECT * FROM users 
                WHERE id = $1
                "#,
                workos_user.id
            )
            .fetch_optional(&mut *transaction)
            .await?
            {
                let updated_user = User {
                    id: existing_user.id,
                    first_name: workos_user.first_name.clone(),
                    last_name: workos_user.last_name.clone(),
                    email: workos_user.email.clone(),
                    created_at: workos_user.created_at,
                    updated_at: Utc::now(),
                    linked_to_keywords: existing_user.linked_to_keywords, // Use existing field value
                };

                query!(
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
                .execute(&mut *transaction)
                .await?;

                user_results.push(updated_user);
            } else {
                let new_user = User::new(
                    &workos_user.id,
                    workos_user.first_name.as_deref().unwrap_or_default(),
                    workos_user.last_name.as_deref().unwrap_or_default(),
                    &workos_user.email,
                    Some(workos_user.created_at),
                );

                query!(
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
                .execute(&mut *transaction)
                .await?;

                user_results.push(new_user);
            }
        }

        transaction.commit().await?;

        Ok(user_results)
    }
}
