// routes/memory.rs

use actix_web::{post, get, put, delete, web, HttpResponse, Responder};
use crate::middleware::auth::AuthenticatedUser;
use crate::models::memory::Memory;
use crate::models::{Chat, MemoryPrompt, Message};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionNamedToolChoice, ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolChoiceOption, ChatCompletionToolType, CreateChatCompletionRequestArgs, FunctionName, FunctionObjectArgs
};
use std::collections::{HashMap, HashSet};
use async_openai::Client;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, error};
use uuid::Uuid;
use crate::AppState;
use crate::AppConfig;
use crate::types::{AddMemoryPromptRequest, CreateMemoryRequest, DeleteMemoryRequest, GenerateMemoriesRequest, GetAllMemoriesQuery, UpdateMemoryRequest};

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[allow(dead_code)]
pub async fn process_memory(
    pool: &PgPool,
    user_id: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    client: Client<OpenAIConfig>,
    memory_prompt_id: Uuid,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch all memories for the user
    let user_memories = Memory::get_all_memories(pool, user_id, Some(memory_prompt_id)).await?;
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
) -> Result<Vec<Memory>, Box<dyn std::error::Error + Send + Sync>> {
    let function_args: serde_json::Value = args.parse()?;

    match name {
        "create_memory" => {
            let memory = function_args["memory"].as_str().unwrap();
            let new_memory = Memory::add_memory(pool, memory, user_id, memory_prompt_id).await?;
            
            Ok(vec![new_memory])
        }
        "update_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let new_memory = function_args["new_memory"].as_str().unwrap();
            let updated_memory =
                Memory::update_memory(pool, memory_id, new_memory, user_id).await?;
            
            Ok(vec![updated_memory])
        }
        "delete_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let deleted_memory = Memory::delete_memory(pool, memory_id, user_id).await?;
            
            Ok(vec![deleted_memory])
        }
        "parse_memories" => {
            // info!("Function args: {:?}", function_args);
            let memory_strings = function_args["memories"].as_array().unwrap();
            let mut memories: Vec<Memory> = Vec::new();

            for memory in memory_strings {
                memories.push(
                    Memory::add_memory(pool, memory.as_str().unwrap(), user_id, memory_prompt_id).await?
                );
            }

            Ok(memories)
        }
        _ => Err("Unknown function".into()),
    }
}

#[allow(dead_code)]
pub async fn get_all_user_memories(
    pool: Arc<PgPool>,
    user_id: &str,
    memory_prompt_id: Option<Uuid>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch all memories with a given memory prompt for the user
    let user_memories = Memory::get_all_memories(&pool, user_id, memory_prompt_id).await?;
    let formatted_memories = Memory::format_memories(user_memories);

    Ok(formatted_memories)
}


// create
#[post("/create")]
async fn create_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<CreateMemoryRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let memory = Memory::add_memory(&app_state.pool, &req_body.content, &authenticated_user.user_id, req_body.memory_prompt_id)
        .await
        .map_err(|e| {
            error!("Failed to create memory: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(HttpResponse::Ok().json(memory))
}

// read
#[get("/get_all")]
async fn get_all_memories(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    info: web::Query<GetAllMemoriesQuery>,
) -> Result<impl Responder, actix_web::Error> {
    let memories = Memory::get_all_memories(&app_state.pool, &authenticated_user.user_id, info.memory_prompt_id)
        .await
        .map_err(|e| {
            error!("Failed to get memories: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(HttpResponse::Ok().json(memories))
}

// update
#[put("/update")]
async fn update_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<UpdateMemoryRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let memory = Memory::update_memory(&app_state.pool, req_body.memory_id, &req_body.content, &authenticated_user.user_id)
        .await
        .map_err(|e| {
            error!("Failed to get memories: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(HttpResponse::Ok().json(memory))
}

// delete
#[delete("/delete")]
async fn delete_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<DeleteMemoryRequest>,
) -> Result<impl Responder, actix_web::Error> {
    Memory::delete_memory(&app_state.pool, req_body.memory_id, &authenticated_user.user_id)
        .await
        .map_err(|e| {
            error!("Failed to get memories: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/add_memory_prompt")]
async fn add_memory_prompt(
    app_state: web::Data<Arc<AppState>>,
    _app_config: web::Data<Arc<AppConfig>>,
    req_body: web::Json<AddMemoryPromptRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let memory_prompt = MemoryPrompt::new(
        &app_state.pool,
        &req_body.prompt,
        req_body.example.clone(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to add memory prompt: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(HttpResponse::Ok().json(memory_prompt))
}

#[post("/generate_from_chat")]
async fn generate_memories_from_chat_history(
    app_state: web::Data<Arc<AppState>>,
    _app_config: web::Data<Arc<AppConfig>>,
    req_body: web::Json<GenerateMemoriesRequest>,
) -> Result<impl Responder, actix_web::Error> {

    let user_id = req_body.user_id.clone();

    let user_chats = Chat::get_chats_for_user(&app_state.pool, &user_id)
        .await
        .map_err(|e| {
            error!("Failed to get user chats: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
    
    let n_samples = match req_body.n_samples {
        Some(n) => std::cmp::min(user_chats.len(), n as usize),
        None => user_chats.len()
    };
        
    let mut samples_dict: HashMap<Uuid, Vec<Message>> = HashMap::new();
    let mut total_samples = 0;
    let mut chats_to_process: Vec<Uuid> = user_chats.iter().map(|chat| chat.id).collect();

    // Process initial messages for each chat
    while !chats_to_process.is_empty() && total_samples < n_samples as usize {
        let chat_id = chats_to_process.pop().unwrap();
        let chat_messages = Message::get_messages_by_chat_id(&app_state.pool, chat_id)
            .await
            .map_err(|e| {
                error!("Failed to get messages for chat: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?;
        
        // if chat_messages count <= 2, continue (skip it)
        if chat_messages.len() > 2 {
            let selected_messages: Vec<Message> = chat_messages
                .into_iter()
                .take(10)
                .collect();

            let added_count = selected_messages.len();
            samples_dict.insert(chat_id, selected_messages);
            total_samples += added_count;
        }
    }

    let mut exhausted_chats: HashSet<Uuid> = HashSet::new();

    // Continue adding messages until we reach n_samples
    while total_samples < n_samples as usize && exhausted_chats.len() < samples_dict.len() {
        for (chat_id, chat_messages) in samples_dict.iter_mut() {
            if total_samples >= n_samples as usize {
                break;
            }
            if exhausted_chats.contains(chat_id) {
                continue;
            }
            match Message::get_next_msg(&app_state.pool, *chat_id, chat_messages.last().unwrap()).await {
                Ok(Some(next_msg)) => {
                    chat_messages.push(next_msg);
                    total_samples += 1;
                },
                Ok(None) => {
                    exhausted_chats.insert(*chat_id);
                },
                Err(e) => {
                    error!("Failed to get next message: {:?}", e);
                    return Err(actix_web::error::ErrorInternalServerError(e));
                }
            }
        }
    }

    // Add this after populating samples_dict
    // debug
    for (chat_id, messages) in &samples_dict {
        println!("Chat ID: {}, Number of messages: {}", chat_id, messages.len());
    }

    let max_ctxt_chars = 200_000;
    let mut memory_ctxt = String::new();
    
    'chat_loop: for (_chat_id, messages) in samples_dict.iter() {
        memory_ctxt.push_str("<begin chat>\n");
        for msg in messages {
            let message_content = format!(
                "<begin message from {}>\n{}\n</end message>\n",
                msg.role, msg.text
            );

            if memory_ctxt.chars().count() + message_content.chars().count() < max_ctxt_chars {
                memory_ctxt.push_str(&message_content);
            } else {
                let remaining_chars = max_ctxt_chars - memory_ctxt.chars().count();
                let truncated_content = message_content.chars().take(remaining_chars).collect::<String>();
                memory_ctxt.push_str(&truncated_content);
                memory_ctxt.push_str("\n</end chat>");

                // Process the current context
                process_memory_context(&app_state, &req_body.user_id, &memory_ctxt, req_body.memory_prompt_id, req_body.log_dir.clone()).await?;

                // Reset the context for the next batch
                memory_ctxt.clear();
                continue 'chat_loop;
            }
        }

        // End the chat if it wasn't ended due to reaching max_ctxt_chars
        memory_ctxt.push_str("\n</end chat>");
    }

    // Process any remaining context
    if !memory_ctxt.is_empty() {
        process_memory_context(&app_state, &req_body.user_id, &memory_ctxt, req_body.memory_prompt_id, req_body.log_dir.clone()).await?;
    }

    Ok(HttpResponse::Ok().finish())
}

async fn process_memory_context(
    app_state: &web::Data<Arc<AppState>>,
    user_id: &str,
    memory_ctxt: &str,
    memory_prompt_id: Uuid,
    log_dir: Option<String>,
) -> Result<(), actix_web::Error> {
    // Print the memory_ctxt
    println!("Processing memory context: {}", memory_ctxt);

    let memory_prompt = MemoryPrompt::get_by_id(&app_state.pool, memory_prompt_id)
        .await
        .map_err(|e| {
            error!("Failed to get memory prompt: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    
    let mut ai_messages: Vec<ChatCompletionRequestMessage> = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content(memory_prompt.prompt.clone())
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
        ChatCompletionRequestSystemMessageArgs::default()
            .content(memory_ctxt.to_string())
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
    ];

    if let Some(example) = &memory_prompt.example {
        ai_messages.insert(1, ChatCompletionRequestAssistantMessageArgs::default()
            .content(example.to_string())
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into());
    }

    let request = CreateChatCompletionRequestArgs::default()
        .model("claude-3-5-sonnet-20240620")
        .messages(ai_messages)
        .build()
        .map_err(|e| {
            error!("Failed to build chat completion request: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let response = app_state.keywords_client
        .chat()
        .create(request)
        .await
        .map_err(|e| {
            error!("Failed to get chat completion response: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let generated_memory = response.choices.first()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("No response from AI"))?
        .message.content.clone()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Empty response from AI"))?;

    println!("Generated memory before saving: {}", generated_memory);

    // Log the memory to a file
    if let Some(log_dir) = log_dir.clone() {
        log_memory(user_id, memory_ctxt, &memory_prompt_id.to_string(), &generated_memory, &log_dir, &memory_prompt.prompt.clone(), memory_prompt.example.as_deref())
            .map_err(|e| {
                error!("Failed to log memory: {:?}", e);    
                actix_web::error::ErrorInternalServerError(e)
            })?;
    }

    let parse_messages: Vec<ChatCompletionRequestMessage> = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content("You will be given a numbered list of memories. Your task is to parse these memoreies into an array of individual memories using the `parse_memories` function.")
            .build()
            .map_err(|e| {
                error!("Failed to build system message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
        ChatCompletionRequestUserMessageArgs::default()
            .content(generated_memory.to_string())
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
    ];

    let parse_request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4o")
        .messages(parse_messages)
        .tools(vec![
            ChatCompletionToolArgs::default()
                .r#type(ChatCompletionToolType::Function)
                .function(
                    FunctionObjectArgs::default()
                        .name("parse_memories")
                        .description("Parse a bullet list of memories into an array of individual memories.")
                        .parameters(json!({
                            "type": "object",
                            "properties": {
                                "memories": {
                                    "type": "array",
                                    "description": "The resulting array of memories",
                                    "items": {
                                        "type": "string",
                                        "description": "A individual memory."
                                    }
                                },
                            },
                            "required": ["memories"],
                        }))
                        .build()
                        .map_err(|e| {
                            error!("Failed to build function: {:?}", e);
                            actix_web::error::ErrorInternalServerError(e)
                        })?,
                )
                .build()
                .map_err(|e| {
                    error!("Failed to build tool: {:?}", e);
                    actix_web::error::ErrorInternalServerError(e)
                })?,
        ])
        .tool_choice(
            ChatCompletionToolChoiceOption::Named(
                ChatCompletionNamedToolChoice {
                    r#type: ChatCompletionToolType::Function,
                    function: FunctionName {
                        name: "parse_memories".to_string(),
                    },
                }
            )
        )
        .build()
        .map_err(|e| {
            error!("Failed to build chat completion request: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let parse_response = app_state.keywords_client
        .chat()
        .create(parse_request)
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
    // Print the generated memory before saving

    if let Some(tool_calls) = parse_response.tool_calls {
        for tool_call in tool_calls {
            let name = tool_call.function.name.clone();
            let args = tool_call.function.arguments.clone();

            let _memories = call_fn(&app_state.pool, &name, &args, user_id, memory_prompt_id)
                .await
                .map_err(|e| {
                    error!("Failed to parse memories: {:?}", e);
                    actix_web::error::ErrorInternalServerError(e)
                })?
                .clone();
            
            // info!("Memories: {:?}", memories);
        }
    }

    Ok(())
}

fn log_memory(
    user_id: &str, 
    memory_context: &str, 
    memory_prompt_id: &str, 
    generated_memories: &str, 
    dir_path: &str,
    prompt: &str,
    example: Option<&str>,
) -> std::io::Result<()> {
    let log_dir = Path::new(dir_path);
    if !log_dir.exists() {
        std::fs::create_dir_all(log_dir)?;
    }

    let log_file = log_dir.join(format!("{}-{}.txt", user_id, memory_prompt_id));
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(log_file)?;

    writeln!(file, "user_id=\"{}\"", user_id)?;
    writeln!(file, "memory_prompt=\"{}\"", prompt)?;
    if let Some(example) = example {
        writeln!(file, "example=\"{}\"", example)?;
    }
    writeln!(file, "memory_context=\"{}\"", memory_context)?;
    writeln!(file, "memory_prompt_id=\"{}\"", memory_prompt_id)?;
    writeln!(file, "generated_memories=\"{}\"", generated_memories)?;
    writeln!(file, "\n")?;

    Ok(())
}

