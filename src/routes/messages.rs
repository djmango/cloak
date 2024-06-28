use actix_web::{post, web, HttpResponse, Responder};
use std::sync::Arc;
use tracing::error;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::message::Message;
use crate::models::MemoryPrompt;
use crate::types::{DownvoteMessageRequest, UpvoteMessageRequest};
use crate::AppState;


#[post("/upvote")]
async fn upvote_message(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<UpvoteMessageRequest>,
) -> Result<impl Responder, actix_web::Error>{
    Message::upvote(&app_state.pool, req_body.message_id, &authenticated_user.user_id)
        .await
        .map_err(|e| {
            error!("Error upvoting message: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;
    
    let message = Message::get_by_id(&app_state.pool, req_body.message_id)
        .await
        .map_err(|e| {
            error!("Error getting message: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;

    if let Some(memory_prompt_id) = message.memory_prompt_id {  
        MemoryPrompt::upvote(&app_state.pool, memory_prompt_id)
            .await
        .map_err(|e| {
            error!("Error upvoting memory prompt: {:?}", e);
                actix_web::error::ErrorInternalServerError(e.to_string())
            })?;
    }

    Ok(HttpResponse::Ok().finish())
}

#[post("/downvote")]
async fn downvote_message(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<DownvoteMessageRequest>,
) -> Result<impl Responder, actix_web::Error>{
    Message::downvote(&app_state.pool, req_body.message_id, &authenticated_user.user_id)
        .await
        .map_err(|e| {
            error!("Error downvoting message: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;

    let message = Message::get_by_id(&app_state.pool, req_body.message_id)
        .await
        .map_err(|e| {
            error!("Error getting message: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;

    if let Some(memory_prompt_id) = message.memory_prompt_id {  
        MemoryPrompt::downvote(&app_state.pool, memory_prompt_id)
            .await
            .map_err(|e| {
                error!("Error downvoting memory prompt: {:?}", e);
                actix_web::error::ErrorInternalServerError(e.to_string())
            })?;
    }
    
    Ok(HttpResponse::Ok().finish())
}