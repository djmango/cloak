use actix_web::{post, web, Error, HttpResponse, Responder};
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::{CreateChatCompletionRequest, CreateChatCompletionStreamResponse};
use async_openai::Client;
// use aws_sdk_bedrockruntime::error::SdkError;
// use aws_sdk_bedrockruntime::operation::invoke_model_with_response_stream::{
//     InvokeModelWithResponseStreamError, InvokeModelWithResponseStreamOutput,
// };
// use aws_sdk_bedrockruntime::primitives::Blob;
// use aws_smithy_runtime_api::http::response::Response;
use bytes::Bytes;
use futures::stream::StreamExt;
use serde_json::to_string;
use std::sync::Arc;
use tracing::info;

use crate::middleware::auth::AuthenticatedUser;
use crate::AppState;

#[post("/v1/chat/completions")]
async fn chat(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<CreateChatCompletionRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let user_id = &authenticated_user.user_id;

    info!(
        "User {} hit the AI endpoint with model: {}",
        user_id, req_body.model
    );

    let mut request_args = req_body.into_inner();

    // For now, we only support streaming completions
    request_args.stream = Some(true);

    // If we want to use claude, use the openrouter client, otherwise use the standard openai client
    let client: Client<OpenAIConfig> = match request_args.model.as_str() {
        "anthropic/claude-3-opus:beta" => app_state.openrouter_client.clone(),
        "anthropic/claude-3-sonnet:beta" => app_state.openrouter_client.clone(),
        "google/gemini-pro" => app_state.openrouter_client.clone(),
        "google/gemini-pro-vision" => app_state.openrouter_client.clone(),
        _ => app_state.oai_client.clone(),
    };

    // Ensure we have at least one message, else return an error
    if request_args.messages.is_empty() {
        return Err(actix_web::error::ErrorBadRequest(
            "At least one message is required",
        ));
    }

    // Truncate messages
    // Include the last x messages, where x is the number of messages we want to keep
    let num_messages: i32 = match request_args.model.as_str() {
        "gpt-4-vision-preview" => 1,
        "google/gemini-pro-vision" => 1,
        _ => 5,
    };

    if request_args.messages.len() > num_messages as usize {
        request_args.messages = request_args
            .messages
            .split_off(request_args.messages.len() - num_messages as usize);
    }

    // Max tokens as 2048
    request_args.max_tokens = Some(2048);

    let response = client
        .chat()
        .create_stream(request_args)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Construct a streaming HTTP response
    let stream: futures::stream::BoxStream<Result<Bytes, Error>> = response
        .map(
            |item_result: Result<CreateChatCompletionStreamResponse, OpenAIError>| match item_result
            {
                Ok(item) => to_string(&item)
                    .map_err(actix_web::error::ErrorInternalServerError)
                    .map(|json_string| Bytes::from(format!("data: {}\n\n", json_string))),
                Err(e) => Err(actix_web::error::ErrorInternalServerError(e.to_string())),
            },
        )
        .boxed();

    Ok(HttpResponse::Ok()
        .content_type("application/stream+json")
        .streaming(stream))
}
