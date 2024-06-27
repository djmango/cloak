use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct GenerateMemoriesRequest {
    pub user_id: String,
    pub memory_prompt_id: Uuid,
    pub n_samples: Option<u8>,
}