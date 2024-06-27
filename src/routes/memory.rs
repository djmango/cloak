// routes/memory.rs

use actix_web::{post, web, HttpResponse, Responder};
use crate::models::memory::Memory;
use crate::models::message::Role;
use crate::models::{Chat, MemoryPrompt, Message};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolType, CreateChatCompletionRequestArgs, FunctionObjectArgs
};
use async_openai::Client;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, error};
use uuid::Uuid;
use crate::AppState;
use crate::AppConfig;
use crate::types::GenerateMemoriesRequest;
use rand::seq::SliceRandom;
use rand::Rng;

pub async fn process_memory(
    pool: &PgPool,
    user_id: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    client: Client<OpenAIConfig>,
    memory_prompt_id: Uuid,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch all memories for the user
    let user_memories = Memory::get_all_memories(pool, user_id, memory_prompt_id).await?;
    let formatted_memories = Memory::format_memories(user_memories);

    info!("User memories: {}", formatted_memories);
    info!("Messages: {:?}", messages);

    // Prepare the messages for the AI, including the formatted memories
    let mut ai_messages = vec![ChatCompletionRequestSystemMessageArgs::default()
        .content(format!(
            "You are an AI assistant with access to the following user memories:\n{}",
            formatted_memories
        ))
        .build()?
        .into()];
    ai_messages.extend(messages);

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u32)
        .model("claude-3-5-sonnet-20240620")
        .messages(ai_messages)
        .tools(vec![
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
                        .build()?,
                )
                .build()?,
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
                        .build()?,
                )
                .build()?,
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
                        .build()?,
                )
                .build()?,
        ])
        .build()?;

    let response_message = client
        .chat()
        .create(request)
        .await?
        .choices
        .first()
        .unwrap()
        .message
        .clone();

    if let Some(tool_calls) = response_message.tool_calls {
        for tool_call in tool_calls {
            let name = tool_call.function.name.clone();
            let args = tool_call.function.arguments.clone();

            call_fn(pool, &name, &args, user_id, memory_prompt_id).await?;
        }
    }

    // Return the content of the response message
    let response_content = response_message.content.unwrap_or_default();
    info!("Memory response content: {}", response_content);
    Ok(response_content)
}

async fn call_fn(
    pool: &PgPool,
    name: &str,
    args: &str,
    user_id: &str,
    memory_prompt_id: Uuid,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let function_args: serde_json::Value = args.parse()?;

    match name {
        "create_memory" => {
            let memory = function_args["memory"].as_str().unwrap();
            let new_memory = Memory::add_memory(pool, memory, user_id, memory_prompt_id).await?;
            Ok(json!({
                "status": "success",
                "memory_id": new_memory.id,
                "message": "Memory created successfully."
            }))
        }
        "update_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let new_memory = function_args["new_memory"].as_str().unwrap();
            let updated_memory =
                Memory::update_memory(pool, memory_id, new_memory, user_id).await?;
            Ok(json!({
                "status": "success",
                "memory_id": updated_memory.id,
                "message": "Memory updated successfully."
            }))
        }
        "delete_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            Memory::delete_memory(pool, memory_id, user_id).await?;
            Ok(json!({
                "status": "success",
                "memory_id": memory_id,
                "message": "Memory deleted successfully."
            }))
        }
        "generate_memories" => {
            // info!("Function args: {:?}", function_args);
            let generalizations = function_args["generalizations"].as_str().unwrap();
            let memories = function_args["memories"].as_array().unwrap();

            for memory in memories {
                Memory::add_memory(pool, memory.as_str().unwrap(), user_id, memory_prompt_id).await?;
            }

            Ok(json!({
                "status": "success",
                "generalizations": generalizations,
                "memories": memories,
                "message": "Memories added successfully"
            }))
        }
        _ => Err("Unknown function".into()),
    }
}

pub async fn get_all_user_memories(
    pool: Arc<PgPool>,
    user_id: &str,
    memory_prompt_id: Uuid,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch all memories with a given memory prompt for the user
    let user_memories = Memory::get_all_memories(&pool, user_id, memory_prompt_id).await?;
    let formatted_memories = Memory::format_memories(user_memories);

    Ok(formatted_memories)
}

#[post("/generate_from_chat")]
async fn generate_memories_from_chat_history(
    app_state: web::Data<Arc<AppState>>,
    _app_config: web::Data<Arc<AppConfig>>,
    req_body: web::Json<GenerateMemoriesRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let client = app_state.keywords_client.clone();
    
    let memory_prompt_id = req_body.memory_prompt_id.clone();
    let memory_prompt = MemoryPrompt::get_by_id(&app_state.pool, memory_prompt_id)
        .await
        .map_err(|e| {
            error!("Failed to get memory prompt: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let user_id = req_body.user_id.clone();

    let user_chats = Chat::get_chats_for_user(&app_state.pool, &user_id)
        .await
        .map_err(|e| {
            error!("Failed to get user chats: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    
    let n_samples = match req_body.n_samples.clone() {
        Some(n) => std::cmp::min(user_chats.len() as u8, n),
        None => std::cmp::min(user_chats.len() as u8, 5)
    };

    let mut rng = rand::thread_rng();
    let chats: Vec<Chat> = user_chats
        .choose_multiple(&mut rng, n_samples as usize)  
        .cloned() 
        .collect();
    
    let mut samples: Vec<Vec<Message>> = Vec::new();

    for chat in chats {
        let all_messages: Vec<Message> = Message::get_messages_by_chat_id(&app_state.pool, chat.id)
            .await
            .map_err(|e| {
                error!("Failed to get messages for chat: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?;
        
        let user_messages: Vec<Message> = all_messages
            .iter()
            .filter(|m| m.role == Role::User)
            .cloned()
            .collect();

        samples.push(user_messages.into_iter().take(5).collect());
    }

    info!("Samples: {:?}", n_samples);

    // Iteravively run the memory prompt on each sample
    for sample in samples {
        let ai_messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessageArgs::default()
            .content(
                format!("{}", memory_prompt.prompt)
            )
            .build()
            .map_err(|e| {
                error!("Failed to build system message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
        ChatCompletionRequestUserMessageArgs::default()
            .content(format!(
                "Here is a list of memories that you have already generated for the user:\n\"\"\"{}\"\"\"\n\nHere is a list of the user's chat messages:\n\"\"\"{}\"\"\"\n\n",
                get_all_user_memories(Arc::new(app_state.pool.clone()), &user_id, memory_prompt_id)
                    .await
                    .map_err(|e| {
                        error!("Failed to get user memories: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e)
                    })?,
                sample.iter().map(|m| m.text.clone()).collect::<Vec<String>>().join("\n\n"),

            ))
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            // .max_tokens(512u32)
            .model("claude-3-5-sonnet-20240620")
            .messages(ai_messages)
            .tools(vec![
                ChatCompletionToolArgs::default()
                    .r#type(ChatCompletionToolType::Function)
                    .function(
                        FunctionObjectArgs::default()
                            .name("generate_memories")
                            .description("Create a new memory based on the user's input.")
                            .parameters(json!({
                                "type": "object",
                                "properties": {
                                    "generalizations": {
                                        "type": "string",
                                        "description": "a list of generalizations made about the user's skills, interests, and personality",
                                    },
                                    "memories": {
                                        "type": "array",
                                        "description": "a list of single short sentence descriptions of one user trait",
                                        "items": {
                                            "type": "string"
                                        }
                                    }
                                },
                                "required": ["generalizations", "memoriess"],
                            }))
                            .build()
                            .map_err(|e| {
                                error!("Failed to build function: {:?}", e);
                                actix_web::error::ErrorInternalServerError(e)
                            })?,
                    )
                    .build()
                    .map_err(|e| {
                        error!("Failed to build tool call: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e)
                    })?,
                ])
                .build()
                .map_err(|e| {
                    error!("Failed to build chat completion request: {:?}", e);
                    actix_web::error::ErrorInternalServerError(e)
                })?;

        let response = client
            .chat()
            .create(request)
            .await
            .map_err(|e| {
                error!("Failed to get chat completion response: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .choices
            .first()
            .unwrap()
            .message
            .clone();

        // info!("Memory response content: {:?}", response);

        if let Some(tool_calls) = response.tool_calls {
            for tool_call in tool_calls {
                let name = tool_call.function.name.clone();
                let args = tool_call.function.arguments.clone();

                call_fn(&app_state.pool, &name, &args, user_id.as_str(), memory_prompt_id)
                    .await
                    .map_err(|e| {
                        error!("Failed to save memories: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e)
                    })?;
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}