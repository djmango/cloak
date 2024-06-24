use crate::models::{Chat, File, Message};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct AllResponse {
    pub chats: Vec<Chat>,
    pub messages: Vec<Message>,
    pub files: Vec<File>,
}
