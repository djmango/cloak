use serde::{Deserialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateMemoryRequest {
    // pub user_id: String,
    pub memory_prompt_id: Uuid,
    pub content: String,
}

#[derive(Deserialize)]
pub struct GetAllMemoriesQuery {
    // pub user_id: String,
    pub memory_prompt_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct UpdateMemoryRequest {
    // pub user_id: String,
    pub memory_id: Uuid,
    pub content: String,
}

#[derive(Deserialize)]
pub struct DeleteMemoryRequest {
    // pub user_id: String,
    pub memory_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct GenerateMemoriesRequest {
    pub user_id: String,
    pub memory_prompt_id: Uuid,
    pub n_samples: Option<u8>,
}

#[derive(Deserialize)]
pub struct AddMemoryPromptRequest {
    pub prompt: String
}