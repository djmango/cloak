use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::{Chat, File, Memory, Message};

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct AllResponse {
    pub chats: Vec<Chat>,
    pub messages: Vec<Message>,
    pub files: Vec<File>,
    pub memories: Vec<Memory>,
}
