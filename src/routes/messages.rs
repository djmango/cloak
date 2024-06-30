use actix_web::{put, web, HttpResponse, Responder};
use uuid::Uuid;
use std::sync::Arc;
use tracing::error;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::message::Message;
use crate::AppState;


#[put("/{message_id}/upvote")]
async fn upvote_message(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    message_id: web::Path<Uuid>,
) -> Result<impl Responder, actix_web::Error>{
    Message::upvote(&app_state.pool, message_id.into_inner(), &authenticated_user.user_id)
        .await
        .map_err(|e| {
            error!("Error upvoting message: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;

    Ok(HttpResponse::Ok().finish())
}

#[put("/{message_id}/downvote")]
async fn downvote_message(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    message_id: web::Path<Uuid>,
) -> Result<impl Responder, actix_web::Error>{
    Message::downvote(&app_state.pool, message_id.into_inner(), &authenticated_user.user_id)
        .await
        .map_err(|e| {
            error!("Error downvoting message: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;
    
    Ok(HttpResponse::Ok().finish())
}