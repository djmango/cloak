use serde::Deserialize;
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
    pub max_samples: Option<u32>,
    pub samples_per_query: Option<u32>,
    pub overlap: Option<u32>,
    pub log_dir: Option<String>,
}

#[derive(Deserialize)]
pub struct AddMemoryPromptRequest {
    pub prompt: String,
    pub example: Option<String>,
}