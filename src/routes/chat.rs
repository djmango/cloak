use crate::middleware::auth::AuthenticatedUser;
use crate::models::chat::Chat;
use crate::AppState;
use actix_web::{delete, put, web, Error, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

#[derive(Deserialize)]
struct UpdateChatRequest {
    name: String,
}

#[put("/{chat_id}")]
async fn update_chat(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    chat_id: web::Path<Uuid>,
    web::Json(update_chat_request): web::Json<UpdateChatRequest>,
) -> Result<HttpResponse, Error> {
    let chat = Chat::update_name(
        &app_state.pool,
        chat_id.into_inner(),
        &authenticated_user.user_id,
        &update_chat_request.name,
    )
    .await
    .map_err(|e| {
        error!("Failed to update chat: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    Ok(HttpResponse::Ok().json(chat))
}

#[delete("/{chat_id}")]
async fn delete_chat(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    chat_id: web::Path<Uuid>,
) -> Result<HttpResponse, Error> {
    Chat::delete(
        &app_state.pool,
        chat_id.into_inner(),
        &authenticated_user.user_id,
    )
    .await
    .map_err(|e| {
        error!("Failed to delete chat: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    Ok(HttpResponse::NoContent().finish())
}
