use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct GenerateMemoriesRequest {
    pub user_id: String,
}