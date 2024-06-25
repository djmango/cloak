use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct UpdateChatRequest {
    pub name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AutorenameChatRequest {
    pub text: String,
}
