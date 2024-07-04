use actix_web::{get, web};
use sqlx::query_as;
use std::sync::Arc;
use tokio::join;
use tracing::error;
use utoipa::OpenApi;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::file::Filetype;
use crate::models::message::Role;
use crate::models::{Chat, File, Memory, Message};
use crate::types::AllResponse;
use crate::AppState;

#[derive(OpenApi)]
#[openapi(paths(sync_all), components(schemas(AllResponse)))]
pub struct ApiDoc;

/// Return all the chats and messages for the user
#[utoipa::path(
    get,
    responses((status = 200, description = "All chats and messages for the user", body = AllResponse, content_type = "application/json"))
)]
#[get("/all")]
pub async fn sync_all(
    app_state: web::Data<Arc<AppState>>,
    user: AuthenticatedUser,
) -> Result<web::Json<AllResponse>, actix_web::Error> {
    let user_id = user.user_id.clone();

    let chats_future = query_as!(
        Chat,
        r#"
        SELECT * FROM chats
        WHERE user_id = $1 AND deleted_at IS NULL
        "#,
        user_id
    )
    .fetch_all(&app_state.pool);

    let messages_future = query_as!(
        Message,
        r#"
        SELECT id, chat_id, user_id, text, role as "role: Role", regenerated, model_id, created_at, updated_at, memory_ids, upvoted, memory_prompt_id FROM messages
        WHERE user_id = $1 AND chat_id IN (SELECT id FROM chats WHERE user_id = $1 AND deleted_at IS NULL)
        "#,
        user_id
    )
    .fetch_all(&app_state.pool);

    let files_future = query_as!(
        File,
        r#"
        SELECT id, chat_id, user_id, message_id, filetype as "filetype: Filetype", show_to_user, url, created_at, updated_at FROM files
        WHERE user_id = $1 AND chat_id IN (SELECT id FROM chats WHERE user_id = $1 AND deleted_at IS NULL)
        "#,
        user_id
    )
    .fetch_all(&app_state.pool);

    let memory_future = query_as!(
        Memory,
        r#"
        SELECT * FROM memories
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_all(&app_state.pool);

    let (chats_result, messages_result, files_result, memories_result) =
        join!(chats_future, messages_future, files_future, memory_future);

    let chats = chats_result.map_err(|e| {
        error!("Failed to fetch chats: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    let messages = messages_result.map_err(|e| {
        error!("Failed to fetch messages: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    let files = files_result.map_err(|e| {
        error!("Failed to fetch files: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    let memories = memories_result.map_err(|e| {
        error!("Failed to fetch memories: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(web::Json(AllResponse {
        chats,
        messages,
        files,
        memories,
    }))
}
