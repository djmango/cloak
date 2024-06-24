use serde::Deserialize;

#[derive(Deserialize)]
pub struct UpdateChatRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct AutorenameChatRequest {
    pub text: String,
}
