use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Chat {
    pub id: Uuid,
    pub user_id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Chat {
    pub fn new(user_id: &str, name: &str) -> Self {
        Chat {
            user_id: user_id.to_string(),
            name: name.to_string(),
            ..Default::default()
        }
    }
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            id: Uuid::new_v4(),
            user_id: String::new(),
            name: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
