use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String, // user_01E4ZCR3C56J083X43JQXF3JK5
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct Chat {
    pub id: Uuid,
    pub user_id: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize)]
// #[sqlx(type_name = "role_enum")] // SQL type name
// #[sqlx(rename_all = "lowercase")] // SQL value name
pub enum Role {
    Assistant,
    System,
    Tool,
    User,
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub user_id: String,
    pub text: String,
    pub role: Role,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub images: Option<Vec<Vec<u8>>>,
}
