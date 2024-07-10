use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateMemoryRequest {
    pub memory_prompt_id: Option<Uuid>,
    pub content: String,
    pub grouping: Option<String>
}

#[derive(Deserialize)]
pub struct GetAllMemoriesQuery {
    // pub user_id: String,
    pub memory_prompt_id: Option<Uuid>,
    // pub format: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateMemoryRequest {
    pub content: String,
    pub grouping: Option<String>
}

#[derive(Deserialize, ToSchema)]
pub struct GenerateMemoriesRequest {
    pub user_id: String,
    pub memory_prompt_id: Uuid,
    pub max_samples: Option<u32>,
    pub samples_per_query: Option<u32>,
    // pub log_dir: Option<String>,
    pub range: Option<(u32, u32)>,
}

#[derive(Deserialize)]
pub struct AddMemoryPromptRequest {
    pub prompt: String,
    pub example: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteAllMemoriesRequest {
    pub user_id: String,
}
