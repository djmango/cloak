// routes/memory.rs

use actix_web::{post, get, put, delete, web, HttpResponse, Responder};
use crate::middleware::auth::AuthenticatedUser;
use crate::models::memory::Memory;
use crate::models::{MemoryPrompt, Message};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionNamedToolChoice, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolChoiceOption, ChatCompletionToolType, CreateChatCompletionRequestArgs, FunctionName, FunctionObjectArgs
};
use async_openai::Client;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, warn, error};
use uuid::Uuid;
use crate::AppState;
use crate::AppConfig;
use crate::types::{AddMemoryPromptRequest, CreateMemoryRequest, GenerateMemoriesRequest, GetAllMemoriesQuery, UpdateMemoryRequest};

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
            let new_memory = Memory::add_memory(pool, memory, user_id, Some(memory_prompt_id)).await?;
            
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
                    Memory::add_memory(pool, memory.as_str().unwrap(), user_id, Some(memory_prompt_id)).await?
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
#[put("/{memory_id}")]
async fn update_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    memory_id: web::Path<Uuid>,
    req_body: web::Json<UpdateMemoryRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let memory = Memory::update_memory(&app_state.pool, memory_id.into_inner(), &req_body.content, &authenticated_user.user_id)
        .await
        .map_err(|e| {
            error!("Failed to get memories: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(HttpResponse::Ok().json(memory))
}

// delete
#[delete("/{memory_id}")]
async fn delete_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    memory_id: web::Path<Uuid>,
) -> Result<impl Responder, actix_web::Error> {
    Memory::delete_memory(&app_state.pool, memory_id.into_inner(), &authenticated_user.user_id)
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
) -> Result<web::Json<Vec<Memory>>, actix_web::Error> {

    let user_id = req_body.user_id.clone();

    let mut user_messages = Message::get_messages_by_user_id(&app_state.pool, &user_id)
        .await
        .map_err(|e| {
            error!("Failed to get user messages: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;
 
    let max_samples = match req_body.max_samples {
        Some(n) => n,
        None => user_messages.len() as u32
    };

    let samples_per_query = match req_body.samples_per_query {
        Some(n) => n,
        None => 30
    };
    let overlap = match req_body.overlap {
        Some(n) => n,
        None => 4
    };

    if user_messages.len() as u32 > max_samples {
        user_messages = user_messages.into_iter().take(max_samples as usize).collect();
    }

    user_messages.reverse();

    let mut generated_samples: Vec<String> = Vec::new();
    let mut start_index = 0;

    while start_index < user_messages.len() {
        let end_index = std::cmp::min(start_index + samples_per_query as usize, user_messages.len());
        let sample_slice = user_messages[start_index..end_index].to_vec();

        let mut memory_ctxt = String::new();
        memory_ctxt.push_str("<chat_messages>\n");
    
        for (i, msg) in sample_slice.iter().enumerate() {
            let message_content = format!(
                "<message {}: {}>\n{}\n</message {}: {}>\n",
                i, msg.role, msg.text, i, msg.role
            );
            memory_ctxt.push_str(&message_content);
        }

        memory_ctxt.push_str("</chat_messages>\n");

        generated_samples.push(memory_ctxt);

        if end_index == user_messages.len() {
            break;
        }

        start_index += (samples_per_query - overlap) as usize;
    }
        
    info!("Generated {} samples", generated_samples.len());

    let generated_memories = process_memory_context(&app_state, &user_id, &generated_samples, req_body.memory_prompt_id, req_body.log_dir.clone()).await?;

    Ok(web::Json(generated_memories))
}

async fn process_memory_context(
    app_state: &web::Data<Arc<AppState>>,
    user_id: &str,
    samples: &Vec<String>,
    memory_prompt_id: Uuid,
    log_dir: Option<String>,
) -> Result<Vec<Memory>, actix_web::Error> {
    let memory_prompt = MemoryPrompt::get_by_id(&app_state.pool, memory_prompt_id)
        .await
        .map_err(|e| {
            error!("Failed to get memory prompt: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let mut generated_memories: Vec<String> = Vec::new();

    for (index, sample) in samples.iter().enumerate() {
        info!("Processing sample {} of {}", index + 1, samples.len());

        let ai_messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestUserMessageArgs::default()
            .content(format!("{}\n{}",
                sample,
                memory_prompt.prompt.clone(), 
            ))
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
        ];

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
        
        info!("Generated memory: {:?}", response.choices.first().unwrap().message.content);
        generated_memories.push(
            response.choices.first()
                .ok_or_else(|| actix_web::error::ErrorInternalServerError("No response from AI"))?
                .message.content.clone()
                .ok_or_else(|| actix_web::error::ErrorInternalServerError("Empty response from AI"))?
        );
    }

    // info!("Generated memory before saving: {:?}", generate_memories);

    // Log the memory to a file
    if let Some(log_dir) = log_dir.clone() {
        log_memory(user_id, samples, &memory_prompt_id.to_string(), &generated_memories, &log_dir, &memory_prompt.prompt.clone(), memory_prompt.example.as_deref())
            .map_err(|e| {
                error!("Failed to log memory: {:?}", e);    
                actix_web::error::ErrorInternalServerError(e)
            })?;
    }

    info!("Generated memories: {:?}", generated_memories);

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
            .content(
                generated_memories
                .iter()
                .map(|m| {
                    let collection = m
                        .split("\"\"\"")
                        .nth(1)
                        .unwrap_or("")
                        .to_string();
                    format!("{}\n", collection)
                })
                .collect::<Vec<String>>()
                .join("\n"),
            )
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
    info!("parsed response, before tool_call: {:?}", parse_response.content);

    match parse_response.tool_calls {
        Some(tool_calls) => {
            let mut all_memories = vec![];
            for tool_call in tool_calls {
                let name = tool_call.function.name.clone();
                let args = tool_call.function.arguments.clone();

                let memories = call_fn(&app_state.pool, &name, &args, user_id, memory_prompt_id)
                    .await
                    .map_err(|e| {
                        error!("Failed to parse memories: {:?}", e);
                        actix_web::error::ErrorInternalServerError(e)
                    })?
                    .clone();
                
                all_memories.extend(memories);
            }
            
            info!("Tool call end, total {} memories", all_memories.len());
            Ok(all_memories)
        },
        None => {
            warn!("No memories saved.");
            Ok(vec![])
        }
    }
}

fn log_memory(
    user_id: &str, 
    samples: &Vec<String>, 
    memory_prompt_id: &str, 
    generated_memories: &Vec<String>, 
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
    writeln!(file, "memory_prompt_id=\"{}\"", memory_prompt_id)?;
    writeln!(file, "memory_prompt=\"{}\"", prompt)?;
    if let Some(example) = example {
        writeln!(file, "example=\"{}\"", example)?;
    }
    for (sample, generated_memory) in samples.iter().zip(generated_memories.iter()) {
        writeln!(file, "sample=\"{}\"", sample)?;
        writeln!(file, "generated_memory=\"{}\"", generated_memory)?;
    }
    writeln!(file, "\n")?;

    Ok(())
}

