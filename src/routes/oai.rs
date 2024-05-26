use crate::config::AppConfig;
use crate::middleware::auth::AuthenticatedUser;
use crate::models::chat::Chat;
use crate::models::message::Message;
use crate::AppState;
use actix_web::{post, web, HttpResponse, Responder};
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart,
    ChatCompletionRequestMessageContentPartText, ChatCompletionRequestUserMessageContent,
    CreateChatCompletionRequest,
};
use async_openai::Client;
use bytes::Bytes;
use futures::lock::Mutex;
use futures::stream::StreamExt;
use futures::TryStreamExt;
use serde_json::to_string;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

#[post("/v1/chat/completions")]
async fn chat(
    app_state: web::Data<Arc<AppState>>,
    app_config: web::Data<Arc<AppConfig>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<CreateChatCompletionRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let start_time = chrono::Utc::now();
    debug!(
        "User {} hit the AI endpoint with model: {}",
        &authenticated_user.user_id, req_body.model
    );

    let mut request_args = req_body.into_inner();

    // For now, we only support streaming completions
    request_args.stream = Some(true);

    // Set the user ID
    request_args.customer_identifier = Some(authenticated_user.user_id.clone());

    // If we want to use claude, use the openrouter client, otherwise use the standard openai client
    let client: Client<OpenAIConfig> = app_state.keywords_client.clone();

    // Max tokens as 4096
    request_args.max_tokens = Some(4096);

    // Conform the model id to what's expected by the provider
    request_args.model = match request_args.model.as_str() {
        "perplexity/mixtral-8x7b-instruct" => {
            "openrouter/mistralai/mixtral-8x7b-instruct".to_string()
        }
        "perplexity/sonar-medium-online" => {
            "openrouter/perplexity/llama-3-sonar-large-32k-online".to_string()
        }
        "anthropic/claude-3-opus:beta" => {
            "bedrock/anthropic.claude-3-opus-20240229-v1:0".to_string()
        }
        "claude-3-opus-20240229" => "bedrock/anthropic.claude-3-opus-20240229-v1:0".to_string(),
        "anthropic/claude-3-sonnet:beta" => "claude-3-sonnet-20240229".to_string(),
        "anthropic/claude-3-haiku:beta" => "claude-3-haiku-20240307".to_string(),
        _ => request_args.model,
    };

    // Set fallback models
    request_args.fallback = Some(vec![
        "gpt-4-turbo-2024-04-09".to_string(),
        "claude-3-sonnet-20240229".to_string(),
    ]);

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
        "openrouter/perplexity/llama-3-sonar-large-32k-online" => 5,
        _ => 15,
    };

    if request_args.messages.len() > num_messages as usize {
        request_args.messages = request_args
            .messages
            .split_off(request_args.messages.len() - num_messages as usize);
    }

    // For each message, ensure the content is not empty, put a - at the start of the message if it's empty
    for message in &mut request_args.messages {
        match message {
            ChatCompletionRequestMessage::User(user_message) => match &mut user_message.content {
                ChatCompletionRequestUserMessageContent::Text(text) => {
                    if text.trim().is_empty() {
                        *text = "example blank text".to_string();
                    }
                }
                ChatCompletionRequestUserMessageContent::Array(array) => {
                    if array.iter().all(|part| match part {
                        ChatCompletionRequestMessageContentPart::Text(text) => {
                            text.text.trim().is_empty()
                        }
                        // Consider non-text parts as "effectively empty" for this check
                        _ => true,
                    }) {
                        array.push(
                            ChatCompletionRequestMessageContentPartText {
                                r#type: "text".to_string(),
                                text: "image attached".to_string(),
                            }
                            .into(),
                        );
                    }
                }
            },
            ChatCompletionRequestMessage::Assistant(assistant_message) => {
                if let Some(content) = &assistant_message.content {
                    if content.trim().is_empty() {
                        assistant_message.content = Some("blank".to_string());
                    }
                }
            }
            _ => {}
        }
    }

    // If using bedrock add the customer credentials
    if request_args.model.starts_with("bedrock/") {
        request_args.customer_credentials = Some(HashMap::from_iter(vec![(
            "bedrock".to_string(),
            serde_json::Value::Object(serde_json::Map::from_iter(vec![
                (
                    "aws_access_key_id".to_string(),
                    serde_json::Value::String(app_config.aws_access_key_id.clone()),
                ),
                (
                    "aws_secret_access_key".to_string(),
                    serde_json::Value::String(app_config.aws_secret_access_key.clone()),
                ),
                (
                    "aws_region_name".to_string(),
                    serde_json::Value::String(app_config.aws_region.clone()),
                ),
            ])),
        )]));
    }

    // Get the last message from the request
    let last_message_option = request_args.messages.last().cloned();
    // Clone the user_id for use in the async block
    let user_id = authenticated_user.user_id.clone();
    // Get an optional chat_id from the invisibility field if it exists
    let chat_id = request_args
        .invisibility
        .as_ref()
        .map(|invisibility| invisibility.chat_id);
    // Clone the invisibility metadata for use in the async block
    let invisibility_metadata = request_args.invisibility.clone();

    let response = client
        .chat()
        .create_stream(request_args)
        .await
        .map_err(|e| {
            error!("Error creating chat completion stream: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;

    // This logging has a non-zero cost, but its essentially trival, less than 5 microseconds theorectically
    let response_content = Arc::new(Mutex::new(String::new()));

    // Create a stream that processes the response from llm host and stores the resulting assistant message
    let stream = response
        .take_while(|item_result| match item_result {
            Ok(item) => {
                if let Some(choice) = item.choices.first() {
                    match &choice.finish_reason {
                        Some(_) => {
                            debug!("Chat completion finished");
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
                        debug!("Chat completion stream ended");
                    }
                    _ => {
                        error!("Error in chat completion stream: {:?}", e);
                    }
                }
                futures::future::ready(false)
            }
        })
        .then({
            let response_content = Arc::clone(&response_content);
            move |item_result| {
                let response_content = Arc::clone(&response_content);
                async move {
                    match item_result {
                        Ok(item) => {
                            if let Some(chat_choice_stream) = item.choices.first() {
                                if let Some(new_response_content) =
                                    &chat_choice_stream.delta.content
                                {
                                    let mut content = response_content.lock().await;
                                    content.push_str(new_response_content);
                                }
                            }
                            to_string(&item)
                                .map(|json_string| {
                                    Bytes::from(format!("data: {}\n\n", json_string))
                                })
                                .map_err(anyhow::Error::from)
                        }
                        Err(e) => Err(anyhow::Error::from(e)),
                    }
                }
            }
        })
        .map_err(|e| {
            error!("Error in chat completion stream: {:?}", e);
            e
        })
        .chain(futures::stream::once({
            let response_content_clone = Arc::clone(&response_content);

            async move {
                let content = response_content_clone.lock().await.clone();
                debug!("Stream processing completed");

                // Spawn a new task to store the results in the database asynchonously
                actix_web::rt::spawn({
                    async move {
                        // Determine the chat to use, either from invisibility or by creating new
                        let chat = match Chat::get_or_create_by_user_id_and_chat_id(
                            &app_state.pool,
                            user_id.clone().as_str(),
                            chat_id,
                        )
                        .await
                        {
                            Ok(chat) => chat,
                            Err(e) => {
                                error!("Error getting or creating chat: {:?}", e);
                                return;
                            }
                        };

                        // If the metadata includes a regenerate_from_message_id, mark the messages after that
                        // in the chat as regenerated
                        if let Some(invisibility_metadata) = invisibility_metadata.clone() {
                            info!("Invisibility metadata: {:?}", invisibility_metadata);

                            if let Some(regenerate_from_message_id) =
                                invisibility_metadata.regenerate_from_message_id
                            {
                                info!(
                                    "Marking chat as regenerated from message_id: {:?}",
                                    regenerate_from_message_id
                                );
                                if let Err(e) = Message::mark_regenerated_from_message_id(
                                    &app_state.pool,
                                    regenerate_from_message_id,
                                )
                                .await
                                {
                                    error!("Error marking chat as regenerated: {:?}", e);
                                }
                            }
                        }

                        // Insert into db a message, the last OAI message (prompt). This should always be a user message
                        if let Some(last_oai_message) = last_message_option {
                            match Message::from_oai(
                                &app_state.pool,
                                last_oai_message,
                                chat.id,
                                &user_id.clone(),
                                invisibility_metadata,
                                Some(start_time),
                            )
                            .await
                            {
                                Ok(message) => {
                                    debug!("Message created from OAI message: {:?}", message);
                                }
                                Err(e) => {
                                    error!("Error creating message from OAI message: {:?}", e);
                                }
                            };
                        } else {
                            error!("No messages found in request_args.messages");
                        }

                        if let Err(err) = Message::new(
                            &app_state.pool,
                            chat.id,
                            &chat.user_id,
                            &content,
                            crate::models::message::Role::Assistant,
                        )
                        .await
                        {
                            error!("Failed to create message: {:?}", err);
                        }
                    }
                });

                Ok(Bytes::new()) as Result<Bytes, anyhow::Error>
            }
        }))
        .filter(|result| futures::future::ready(!matches!(result, Ok(bytes) if bytes.is_empty())))
        .boxed();

    let response = HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream);

    Ok(response)
}
