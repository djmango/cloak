use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize)]
pub struct CreateMemoryRequest {
    pub content: String,
    pub grouping: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateMemoryRequest {
    pub content: String,
    pub grouping: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct GenerateMemoriesRequest {
    pub user_id: String,
    pub max_samples: Option<u32>,
    pub samples_per_query: Option<u32>,
    pub range: Option<(u32, u32)>,
}
