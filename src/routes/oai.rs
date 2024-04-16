use actix_web::{post, web, HttpResponse, Responder};
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::CreateChatCompletionRequest;
use async_openai::Client;
use bytes::Bytes;
use futures::stream::StreamExt;
use futures::TryStreamExt;
use serde_json::to_string;
use std::sync::Arc;
use tracing::{error, info};

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

    // Set the user ID
    request_args.customer_identifier = Some(user_id.clone());

    // If we want to use claude, use the openrouter client, otherwise use the standard openai client
    let client: Client<OpenAIConfig> = app_state.keywords_client.clone();

    // Ensure we have at least one message, else return an error
    if request_args.messages.is_empty() {
        return Err(actix_web::error::ErrorBadRequest(
            "At least one message is required",
        ));
    }

    // Truncate messages
    // Include the last x messages, where x is the number of messages we want to keep
    let num_messages: i32 = match request_args.model.as_str() {
        "gpt-4-vision-preview" => 3,
        "perplexity/sonar-medium-online" => 3,
        _ => 7,
    };

    if request_args.messages.len() > num_messages as usize {
        request_args.messages = request_args
            .messages
            .split_off(request_args.messages.len() - num_messages as usize);
    }

    // Max tokens as 2048
    request_args.max_tokens = Some(2048);

    // Conform the model id to what's expected by the provider
    request_args.model = match request_args.model.as_str() {
        "perplexity/mixtral-8x7b-instruct" => {
            "openrouter/mistralai/mixtral-8x7b-instruct".to_string()
        }
        "perplexity/sonar-medium-online" => "openrouter/perplexity/sonar-medium-online".to_string(),
        "anthropic/claude-3-opus:beta" => "openrouter/anthropic/claude-3-opus".to_string(),
        "anthropic/claude-3-sonnet:beta" => "openrouter/anthropic/claude-3-sonnet".to_string(),
        "anthropic/claude-3-haiku:beta" => "openrouter/anthropic/claude-3-haiku".to_string(),
        _ => request_args.model,
    };

    info!("Creating chat completion stream");
    let response = client
        .chat()
        .create_stream(request_args)
        .await
        .map_err(|e| {
            error!("Error creating chat completion stream: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;

    let stream = response
        .take_while(|item_result| match item_result {
            Ok(item) => {
                if let Some(choice) = item.choices.first() {
                    match &choice.finish_reason {
                        Some(_) => {
                            info!("Chat completion finished");
                            return futures::future::ready(false);
                        }
                        None => {}
                    }
                }
                futures::future::ready(true)
            }
            Err(e) => {
                match e {
                    OpenAIError::StreamError(ref err) if err == "Stream ended" => {
                        info!("Chat completion stream ended");
                    }
                    _ => {
                        error!("Error in chat completion stream: {:?}", e);
                    }
                }
                futures::future::ready(false)
            }
        })
        .map(|item_result| match item_result {
            Ok(item) => to_string(&item)
                .map(|json_string| Bytes::from(format!("data: {}\n\n", json_string)))
                .map_err(actix_web::error::ErrorInternalServerError),
            Err(e) => Err(actix_web::error::ErrorInternalServerError(e)),
        })
        .map_err(|e| {
            error!("Error in chat completion stream: {:?}", e);
            e
        })
        .boxed();

    let response = HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream);

    Ok(response)
}
