use crate::middleware::auth::AuthenticatedUser;
use crate::models::chat::Chat;
use crate::models::message::{Message, Role};
use crate::AppState;
use actix_web::{get, web};
use serde::{Deserialize, Serialize};
use sqlx::query_as;
use std::sync::Arc;
use tokio::join;
use tracing::error;

#[derive(Serialize, Deserialize, Debug)]
struct ChatsAndMessagesResponse {
    chats: Vec<Chat>,
    messages: Vec<Message>,
}

/// Return all the chats and messages for the user
#[get("/chats_and_messages")]
pub async fn chats_and_messages(
    app_state: web::Data<Arc<AppState>>,
    user: AuthenticatedUser,
) -> Result<web::Json<ChatsAndMessagesResponse>, actix_web::Error> {
    let user_id = user.user_id.clone();

    let chats_future = query_as!(
        Chat,
        r#"
        SELECT * FROM chats
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_all(&app_state.pool);

    let messages_future = query_as!(
        Message,
        r#"
        SELECT id, chat_id, user_id, text, role as "role: Role", files, created_at, updated_at FROM messages
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_all(&app_state.pool);

    let (chats_result, messages_result) = join!(chats_future, messages_future);

    let chats = chats_result.map_err(|e| {
        error!("Failed to fetch chats: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    let messages = messages_result.map_err(|e| {
        error!("Failed to fetch messages: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(web::Json(ChatsAndMessagesResponse { chats, messages }))
}