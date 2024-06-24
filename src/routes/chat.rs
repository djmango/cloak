use actix_web::{delete, put, web, Error, HttpResponse};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
    ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    CreateChatCompletionRequest,
};
use async_openai::Client;
use sqlx::query_scalar;
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::chat::Chat;
use crate::prompts::Prompts;
use crate::types::{AutorenameChatRequest, UpdateChatRequest};
use crate::AppState;

#[put("/{chat_id}/autorename")]
async fn autorename_chat(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    chat_id: web::Path<Uuid>,
    autorename_chat_request: Option<web::Json<AutorenameChatRequest>>,
) -> Result<web::Json<Chat>, Error> {
    let client: Client<OpenAIConfig> = app_state.keywords_client.clone();
    let user_id = authenticated_user.user_id;
    let chat_id = chat_id.into_inner();

    let message_text = if let Some(autorename_chat_request) = autorename_chat_request {
        autorename_chat_request.text.clone()
    } else {
        // Get the text of the oldest non-regenerated message given the chat id and user id
        let pool = &app_state.pool;

        let result: String = query_scalar!(
            r#"
            SELECT text FROM messages
            WHERE chat_id = $1
            AND user_id = $2
            AND regenerated = false
            ORDER BY created_at ASC
            LIMIT 1
            "#,
            chat_id,
            user_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Database query returned no results: {}", e);
            actix_web::error::ErrorNotFound(e)
        })?;

        result
    };

    let request = CreateChatCompletionRequest {
        messages: vec![
            ChatCompletionRequestMessage::User( ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(Prompts::AUTORENAME_1.to_string()),
                ..Default::default()
            }),
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: Some(Prompts::AUTORENAME_2.to_string()),
                ..Default::default()
            }),
            ChatCompletionRequestMessage::User( ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(Prompts::AUTORENAME_3.to_string()),
                ..Default::default()
            }),
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: Some(Prompts::AUTORENAME_4.to_string()),
                ..Default::default()
            }),
            ChatCompletionRequestMessage::User( ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(Prompts::AUTORENAME_5.to_string()),
                ..Default::default()
            }),
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: Some(Prompts::AUTORENAME_6.to_string()),
                ..Default::default()
            }),
            ChatCompletionRequestMessage::User( ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(format!(
                    "Create a concise, 3-5 word phrase as a header for the following. Please return only the 3-5 word header and no additional words or characters: \"{}\"",
                    message_text
                )),
                ..Default::default()
            },
        )
        ],
        model: "groq/llama3-8b-8192".to_string(),
        max_tokens: Some(64),
        ..Default::default()
    };

    let response = client.chat().create(request).await.map_err(|e| {
        error!("Failed to create chat: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    let name = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .unwrap_or("New Chat".to_string());

    let chat = Chat::update_name(&app_state.pool, chat_id, &user_id, &name)
        .await
        .map_err(|e| {
            error!("Failed to update chat: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    Ok(web::Json(chat))
}

#[put("/{chat_id}")]
async fn update_chat(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    chat_id: web::Path<Uuid>,
    web::Json(update_chat_request): web::Json<UpdateChatRequest>,
) -> Result<web::Json<Chat>, Error> {
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
    Ok(web::Json(chat))
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
