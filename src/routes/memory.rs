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
    ChatCompletionToolArgs, ChatCompletionToolType, CreateChatCompletionRequest,
    FunctionObjectArgs, FunctionCall, ChatCompletionMessageToolCall, FinishReason
};

use async_openai::Client;
use bytes::Bytes;
use futures::lock::Mutex;
use futures::stream::StreamExt;
use futures::TryStreamExt;
use serde_json::{json, to_string, Value};
use std::sync::Arc;
use tracing::{debug, error, info};
use std::collections::HashMap;

#[post("/v1/chat/completions")]
async fn chat_with_memory(
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

    // For now, we only support streaming completions
    request_args.stream = Some(true);

    // Set the user ID
    request_args.customer_identifier = Some(authenticated_user.user_id.clone());

    // If we want to use claude, use the openrouter client, otherwise use the standard openai client
    let client: Client<OpenAIConfig> = app_state.keywords_client.clone();

    // Max tokens as 4096
    request_args.max_tokens = Some(4096);

    // Conform the model id to what's expected by the provider
    request_args.model = "gpt-4o".to_string();

    // Set fallback models
    request_args.fallback = Some(vec![
        "gpt-4-turbo-2024-04-09".to_string(),
        "gpt-3.5-turbo".to_string(),
    ]);

    request_args.tools = Some(vec![
        ChatCompletionToolArgs::default()
            .r#type(ChatCompletionToolType::Function)
            .function(
                FunctionObjectArgs::default()
                    .name("get_current_weather")
                    .description("Get the current weather in a given location")
                    .parameters(json!({
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The city and state, e.g. San Francisco, CA",
                            },
                            "unit": { "type": "string", "enum": ["celsius", "fahrenheit"] },
                        },
                        "required": ["location"],
                    }))
                    .build()
                    .map_err(|e| {
                        error!("Error creating get_current_weather function: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e.to_string())
                    })?,
            )
            .build()
            .map_err(|e| {
                error!("Error creating get_current_weather tool: {:?}", e);
                actix_web::error::ErrorInternalServerError(e.to_string())
            })?,
        ChatCompletionToolArgs::default()
            .r#type(ChatCompletionToolType::Function)
            .function(
                FunctionObjectArgs::default()
                    .name("create_memory")
                    .description("Create a new memory based on the user's input.")
                    .parameters(json!({
                        "type": "object",
                        "properties": {
                            "memory": {
                                "type": "string",
                                "description": "The memory content to be stored.",
                            },
                        },
                        "required": ["memory"],
                    }))
                    .build()
                    .map_err(|e| {
                        error!("Error creating create_memory function: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e.to_string())
                    })?,
            )
            .build()
            .map_err(|e| {
                error!("Error creating create_memory tool: {:?}", e);
                actix_web::error::ErrorInternalServerError(e.to_string())
            })?,
        ChatCompletionToolArgs::default()
            .r#type(ChatCompletionToolType::Function)
            .function(
                FunctionObjectArgs::default()
                    .name("update_memory")
                    .description("Update an existing memory based on the user's input.")
                    .parameters(json!({
                        "type": "object",
                        "properties": {
                            "memory_id": {
                                "type": "string",
                                "description": "The ID of the memory to be updated.",
                            },
                            "new_memory": {
                                "type": "string",
                                "description": "The updated memory content.",
                            },
                        },
                        "required": ["memory_id", "new_memory"],
                    }))
                    .build()
                    .map_err(|e| {
                        error!("Error creating update_memory function: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e.to_string())
                    })?,
            )
            .build()
            .map_err(|e| {
                error!("Error creating update_memory tool: {:?}", e);
                actix_web::error::ErrorInternalServerError(e.to_string())
            })?,
        ChatCompletionToolArgs::default()
            .r#type(ChatCompletionToolType::Function)
            .function(
                FunctionObjectArgs::default()
                    .name("delete_memory")
                    .description("Delete a memory based on the memory ID.")
                    .parameters(json!({
                        "type": "object",
                        "properties": {
                            "memory_id": {
                                "type": "string",
                                "description": "The ID of the memory to be deleted.",
                            },
                        },
                        "required": ["memory_id"],
                    }))
                    .build()
                    .map_err(|e| {
                        error!("Error creating delete_memory function: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e.to_string())
                    })?,
            )
            .build()
            .map_err(|e| {
                error!("Error creating delete_memory tool: {:?}", e);
                actix_web::error::ErrorInternalServerError(e.to_string())
            })?,
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

    let messages_clone = request_args.messages.clone();

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
    
    // Create a hashmap to store tool call states
    let tool_call_states: Arc<Mutex<HashMap<(i32, i32), ChatCompletionMessageToolCall>>> =
        Arc::new(Mutex::new(HashMap::new()));

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
            let tool_call_states = Arc::clone(&tool_call_states);
            move |item_result| {
                let response_content = Arc::clone(&response_content);
                let tool_call_states = Arc::clone(&tool_call_states);
                let messages_clone = messages_clone.clone();
                async move {
                    match item_result {
                        Ok(item) => {
                            if let Some(chat_choice_stream) = item.choices.first() {
                                let function_responses: Arc<
                                    Mutex<Vec<(ChatCompletionMessageToolCall, Value)>>,
                                > = Arc::new(Mutex::new(Vec::new()));
                                if let Some(tool_calls) = &chat_choice_stream.delta.tool_calls {
                                    for (_i, tool_call_chunk) in tool_calls.into_iter().enumerate() {
                                        debug!("Tool call chunk: {:?}", tool_call_chunk);
                                        let key = (chat_choice_stream.index as i32, tool_call_chunk.index);
                                        let states = tool_call_states.clone();
                                        let tool_call_data = tool_call_chunk.clone();

                                        let mut states_lock = states.lock().await;
                                        let state = states_lock.entry(key).or_insert_with(|| {
                                            ChatCompletionMessageToolCall {
                                                id: tool_call_data.id.clone().unwrap_or_default(),
                                                r#type: ChatCompletionToolType::Function,
                                                function: FunctionCall {
                                                    name: tool_call_data
                                                        .function
                                                        .as_ref()
                                                        .and_then(|f| f.name.clone())
                                                        .unwrap_or_default(),
                                                    arguments: tool_call_data
                                                        .function
                                                        .as_ref()
                                                        .and_then(|f| f.arguments.clone())
                                                        .unwrap_or_default(),
                                                },
                                            }
                                        });
                                        if let Some(arguments) = tool_call_chunk
                                            .function
                                            .as_ref()
                                            .and_then(|f| f.arguments.as_ref())
                                        {
                                            state.function.arguments.push_str(arguments);
                                        }
                                    }
                                }
                                if let Some(finish_reason) = &chat_choice_stream.finish_reason {
                                    debug!("Finish reason: {:?}", finish_reason);
                                    if matches!(finish_reason, FinishReason::ToolCalls) {
                                        let tool_call_states_clone = tool_call_states.clone();

                                        let tool_calls_to_process = {
                                            let states_lock = tool_call_states_clone.lock().await;
                                            states_lock
                                                .iter()
                                                .map(|(_key, tool_call)| {
                                                    let name = tool_call.function.name.clone();
                                                    let args = tool_call.function.arguments.clone();
                                                    let tool_call_clone = tool_call.clone();
                                                    (name, args, tool_call_clone)
                                                })
                                                .collect::<Vec<_>>()
                                        };

                                        let mut handles = Vec::new();

                                        for (name, args, tool_call_clone) in tool_calls_to_process {
                                            let response_content_clone = function_responses.clone();
                                            let handle = tokio::spawn(async move {
                                                debug!("Calling function: {}, args: {}", name, args);
                                                let response_content = call_fn(&name, &args).await.unwrap();
                                                let mut function_responses_lock =
                                                    response_content_clone.lock().await;
                                                function_responses_lock
                                                    .push((tool_call_clone, response_content));
                                            });
                                            handles.push(handle);
                                        }
            
                                        for handle in handles {
                                            handle.await.unwrap();
                                        }

                                        let function_responses_clone = function_responses.clone();
                                        let function_responses_lock = function_responses_clone.lock().await;
                                        let mut messages: Vec<ChatCompletionRequestMessage> = messages_clone;
                                    }
                                }
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



async fn call_fn(name: &str, args: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut available_functions: HashMap<&str, fn(&str, &str) -> Value> = HashMap::new();
    available_functions.insert("get_current_weather", get_current_weather);
    available_functions.insert("create_memory", create_memory);
    available_functions.insert("update_memory", update_memory);
    available_functions.insert("delete_memory", delete_memory);

    let function_args: Value = serde_json::from_str(args).unwrap();

    let function_response = match name {
        "get_current_weather" => {
            let location = function_args["location"].as_str().unwrap();
            let unit = function_args["unit"].as_str().unwrap_or("fahrenheit");
            available_functions.get(name).unwrap()(location, unit)
        }
        "create_memory" => {
            let memory = function_args["memory"].as_str().unwrap();
            available_functions.get(name).unwrap()(memory, "")
        }
        "update_memory" => {
            let memory_id = function_args["memory_id"].as_str().unwrap();
            let new_memory = function_args["new_memory"].as_str().unwrap();
            available_functions.get(name).unwrap()(memory_id, new_memory)
        }
        "delete_memory" => {
            let memory_id = function_args["memory_id"].as_str().unwrap();
            available_functions.get(name).unwrap()(memory_id, "")
        }
        _ => return Err("Function not found".into()),
    };

    Ok(function_response)
}

fn get_current_weather(location: &str, unit: &str) -> Value {
    let temperature = 25;
    let forecasts = [
        "sunny", "cloudy", "overcast", "rainy", "windy", "foggy", "snowy",
    ];
    let forecast = forecasts[0];

    let weather_info = json!({
        "location": location,
        "temperature": temperature.to_string(),
        "unit": unit,
        "forecast": forecast
    });

    weather_info
}

fn create_memory(memory: &str, _: &str) -> Value {
    //call create memory function in src/models/memory.rs
    let memory_info = json!({
        "status": "success",
        "message": format!("Memory created: {}", memory),
    });

    memory_info
}

fn update_memory(memory_id: &str, new_memory: &str) -> Value {
    //call update memory function in src/models/memory.rs
    let update_info = json!({
        "status": "success",
        "message": format!("Memory with ID {} updated to: {}", memory_id, new_memory),
    });

    update_info
}

fn delete_memory(memory_id: &str, _: &str) -> Value {
    //call delete memory function in src/models/memory.rs
    let delete_info = json!({
        "status": "success",
        "message": format!("Memory with ID {} deleted", memory_id),
    });

    delete_info
}