use actix_web::{post, web, HttpResponse, Responder};
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::{
    ChatCompletionFunctionCall, ChatCompletionRequestMessage,
    ChatCompletionRequestMessageContentPart, ChatCompletionRequestMessageContentPartText,
    ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessageContent,
    ChatCompletionResponseFormat, ChatCompletionStreamOptions, ChatCompletionTool,
    ChatCompletionToolChoiceOption, CreateChatCompletionRequest, InvisibilityMetadata,
};
use async_openai::Client;
use bytes::Bytes;
use chrono::Utc;
use futures::lock::Mutex;
use futures::stream::StreamExt;
use futures::TryStreamExt;
use serde_json::to_string;
use std::sync::Arc;
use tracing::{debug, error, info};
use utoipa::OpenApi;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::message::Role;
use crate::models::{Chat, Memory, Message};
use crate::routes;
use crate::{prompts::Prompts, AppState};

#[derive(OpenApi)]
#[openapi(
    paths(chat),
    components(schemas(
        CreateChatCompletionRequest,
        ChatCompletionRequestMessage,
        ChatCompletionResponseFormat,
        ChatCompletionStreamOptions,
        ChatCompletionTool,
        ChatCompletionToolChoiceOption,
        ChatCompletionFunctionCall,
        InvisibilityMetadata,
    ))
)]
pub struct ApiDoc;

// Helper function to create the system prompt
async fn create_system_prompt(
    app_state: &web::Data<Arc<AppState>>,
    user_id: &str,
    start_time: chrono::DateTime<chrono::Utc>,
) -> Result<String, actix_web::Error> {
    // Fetch user memories
    let memories = Memory::get_all_memories(&app_state.pool, user_id, &app_state.memory_cache)
        .await
        .map_err(|e| {
            error!("Failed to get memories: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    info!("got {} memories", memories.len());
    // Format memories
    let format_with_id = false;
    let formatted_memories = Memory::format_grouped_memories(&memories, format_with_id);

    // Create the system prompt with datetime and memories
    Ok(Prompts::SYSTEM_PROMPT
        .replace("{0}", &start_time.format("%Y-%m-%d %H:%M:%S").to_string())
        .replace("{1}", &formatted_memories))
}

/// The primary oai mocked streaming chat completion endpoint, with all i.inc features
#[utoipa::path(
    get,
    responses(
        // (status = 200, description = "Chat completion API", body = ChatCompletionResponse, content_type = "application/json"),
        // (status = 200, description = "Chat completion API (streaming)", body = ChatCompletionChunk, content_type = "text/event-stream"),
        (status = 200, description = "Chat completion API",  content_type = "application/json"),
        (status = 200, description = "Chat completion API (streaming)",  content_type = "text/event-stream"),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal Server Error")
    )
)]
#[post("/v1/chat/completions")]
async fn chat(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<CreateChatCompletionRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let start_time = chrono::Utc::now();
    debug!(
        "User {} hit the AI endpoint with model: {}",
        &authenticated_user.user_id, req_body.model
    );

    let mut request_args = req_body.into_inner();

    // Attempt to create the system prompt
    match create_system_prompt(&app_state, &authenticated_user.user_id, start_time).await {
        Ok(system_prompt) => {
            // Log the system prompt
            info!("System prompt created: {}", system_prompt);

            // Prepend the system prompt to the messages
            request_args.messages.insert(
                0,
                ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                    content: system_prompt,
                    name: Some("system".to_string()),
                }),
            );
        }
        Err(e) => {
            // Log the error but continue without the system prompt
            error!(
                "Error creating system prompt: {:?}. Continuing without system prompt.",
                e
            );
        }
    }

    // For now, we only support streaming completions
    request_args.stream = Some(true);

    // Set the user ID
    request_args.customer_identifier = Some(authenticated_user.user_id.clone());

    // If we want to use claude, use the openrouter client, otherwise use the standard openai client
    let client: Client<OpenAIConfig> = app_state.keywords_client.clone();

    // Max tokens as 4096
    request_args.max_tokens = Some(4096);

    // Check if user is rate limited
    if authenticated_user.is_rate_limited() {
        request_args.model = "groq/llama3-70b-8192".to_string();
    } else {
        // Conform the model id to what's expected by the provider
        request_args.model = match request_args.model.as_str() {
            "perplexity/mixtral-8x7b-instruct" => {
                "openrouter/mistralai/mixtral-8x7b-instruct".to_string()
            }
            "perplexity/sonar-medium-online" => {
                "openrouter/perplexity/llama-3-sonar-large-32k-online".to_string()
            }
            "openrouter/google/gemini-pro-1.5" => "gemini-1.5-flash-001".to_string(),
            "claude-3-5-sonnet-20240620" => {
                "bedrock/anthropic.claude-3-5-sonnet-20241022-v2:0".to_string()
            }
            "claude-3-5-sonnet-20241022" => {
                "bedrock/anthropic.claude-3-5-sonnet-20241022-v2:0".to_string()
            }
            "bedrock/anthropic.claude-3-opus-20240229-v1:0" => {
                "bedrock/anthropic.claude-3-5-sonnet-20241022-v2:0".to_string()
            }
            _ => request_args.model,
        };
    }

    info!("Model set to: {}", request_args.model);

    // Set fallback models
    request_args.fallback = Some(vec![
        "gpt-4o".to_string(),
        "claude-3-5-sonnet-20240620".to_string(),
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

    // Remove the invisibility field from the request_args

    // This field is not part of the OpenAI API and is only used internally
    request_args.invisibility = None;

    let model_id = request_args.model.clone();

    // Use the chat id to track threads
    request_args.thread_identifier = chat_id.map(|id| id.to_string());

    let response = client
        .chat()
        .create_stream(request_args)
        .await
        .map_err(|e| {
            error!("Error creating chat completion stream: {:?}", e);
            actix_web::error::ErrorInternalServerError(e.to_string())
        })?;

    // This logging has a non-zero cost, but its essentially trivial, less than 5 microseconds theoretically
    let response_content = Arc::new(Mutex::new(String::new()));

    // Create a stream that processes the response from llm host and stores the resulting assistant message
    let stream = response
        .take_while(|item_result| match item_result {
            Ok(item) => {
                if let Some(choice) = item.choices.first() {
                    if choice.finish_reason.is_some() {
                        debug!("Chat completion finished");
                        return futures::future::ready(false);
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

                // Spawn a new task to store the results in the database asynchronously
                actix_web::rt::spawn({
                    async move {
                        // Determine the chat to use, either from invisibility or by creating new
                        let chat = match Chat::get_or_create_by_user_id_and_chat_id(
                            &app_state.pool,
                            user_id.clone().as_str(),
                            chat_id,
                            match invisibility_metadata.as_ref() {
                                Some(metadata) => metadata.branch_from_message_id,
                                None => None,
                            },
                        )
                        .await
                        {
                            Ok(chat) => chat,
                            Err(e) => {
                                error!("Error getting or creating chat: {:?}", e);
                                return;
                            }
                        };

                        // If the metadata includes a regenerate_from_message_id, mark the messages after that in the chat as regenerated
                        if let Some(metadata) = invisibility_metadata.as_ref() {
                            if let Some(regenerate_from_message_id) =
                                metadata.regenerate_from_message_id
                            {
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

                            let (content, role, files) = match last_oai_message {
                                ChatCompletionRequestMessage::User(user_message) => match user_message.content {
                                    ChatCompletionRequestUserMessageContent::Text(text) => (text, Role::User, vec![]),
                                    ChatCompletionRequestUserMessageContent::Array(array) => {
                                        let mut concatenated_text = String::new();
                                        let mut file_urls = Vec::new();

                                        for part in &array {
                                            match part {
                                                ChatCompletionRequestMessageContentPart::Text(text_part) => {
                                                    if !text_part.text.trim().is_empty() {
                                                        concatenated_text.push_str(&text_part.text);
                                                    }
                                                }
                                                ChatCompletionRequestMessageContentPart::ImageUrl(image_part) => {
                                                    file_urls.push(image_part.image_url.url.clone());
                                                }
                                            }
                                        }
                                        (concatenated_text, Role::User, file_urls)
                                    }
                                },
                                ChatCompletionRequestMessage::Assistant(assistant_message) => {
                                    if let Some(content) = &assistant_message.content {
                                        (content.clone(), Role::Assistant, vec![])
                                    } else {
                                        ("".to_string(), Role::Assistant, vec![])
                                    }
                                },
                                _ => {
                                    error!("Unsupported message type");
                                    return;
                                }
                            };

                            match Message::from_oai(
                                &app_state.pool,
                                content.clone(),
                                role.clone(),
                                files.clone(),
                                chat.id,
                                &user_id.clone(),
                                Some(model_id.clone()),
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

                            if role == Role::User && routes::memory::use_message_for_memory(&app_state, &content).await.unwrap_or(false) {
                                let last_msg_range = (start_time, Utc::now());
                                match routes::memory::generate_memories_from_chat_history(
                                    &app_state,
                                    None,
                                    &user_id,
                                    Some(1),
                                    Some(1),
                                    Some(last_msg_range)
                                )
                                .await
                                {
                                    Ok(memories) => {
                                        info!("Real-time memories generated successfully for user: {}. Count: {}", user_id, memories.len());
                                    },
                                    Err(e) => {
                                        error!("Error generating memories for user {}: {:?}", user_id, e);
                                    }
                                }
                            }
                        } else {
                            error!("No messages found in request_args.messages");
                        }

                        if let Err(err) = Message::new(
                            &app_state.pool,
                            chat.id,
                            &chat.user_id,
                            Some(model_id.clone()),
                            &content,
                            Role::Assistant,
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
