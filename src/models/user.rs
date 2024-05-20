use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String, // user_01E4ZCR3C56J083X43JQXF3JK5
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub linked_to_keywords: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn new(id: &str, first_name: &str, last_name: &str, email: &str) -> Self {
        User {
            id: id.to_string(),
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            email: email.to_string(),
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
}

impl Default for User {
    fn default() -> Self {
        User {
            id: String::new(), // These will be ignored by the `new` constructor
            first_name: String::new(),
            last_name: String::new(),
            email: String::new(),
            linked_to_keywords: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
