// routes/memory.rs

use actix_web::{post, get, put, delete, web, HttpResponse, Responder};
use crate::middleware::auth::AuthenticatedUser;
use crate::models::memory::Memory;
use crate::models::{MemoryPrompt, Message};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestMessage, 
    ChatCompletionRequestUserMessageArgs, 
    CreateChatCompletionRequestArgs, 
};
use async_openai::Client;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, warn, error};
use uuid::Uuid;
use futures::future::join_all;
use crate::AppState;
use crate::AppConfig;
use crate::types::{
    AddMemoryPromptRequest, 
    CreateMemoryRequest, 
    GenerateMemoriesRequest, 
    GetAllMemoriesQuery, 
    UpdateMemoryRequest, 
    DeleteAllMemoriesRequest
};
use crate::prompts::Prompts;
use chrono::Utc;
use tiktoken_rs::cl100k_base;
use moka::future::Cache;
use std::collections::HashMap;

async fn call_fn(
    pool: &PgPool,
    name: &str,
    args: &str,
    user_id: &str,
    memory_prompt_id: Uuid,
    memory_cache: &Cache<String, HashMap<Uuid, Memory>>,
) -> Result<Vec<Memory>, Box<dyn std::error::Error + Send + Sync>> {
    let function_args: serde_json::Value = args.parse()?;

    match name {
        "create_memory" => {
            let memory = function_args["memory"].as_str().unwrap();
            let grouping = function_args.get("grouping").and_then(|g| g.as_str());
            let emoji = function_args.get("emoji").and_then(|e| e.as_str());
            let new_memory = Memory::add_memory(pool, memory, grouping, emoji, user_id, Some(memory_prompt_id), memory_cache).await?;
            
            Ok(vec![new_memory])
        }
        "update_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let new_memory = function_args["new_memory"].as_str().unwrap();
            let grouping = function_args.get("grouping").and_then(|g| g.as_str());
            let emoji = function_args.get("emoji").and_then(|e| e.as_str());
            let updated_memory =
                Memory::update_memory(pool, memory_id, new_memory, grouping, emoji, user_id, memory_cache).await?;
            
            Ok(vec![updated_memory])
        }
        "delete_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let deleted_memory = Memory::delete_memory(pool, memory_id, user_id, memory_cache).await?;
            
            Ok(vec![deleted_memory])
        }
        "parse_memories" => {
            // info!("Function args: {:?}", function_args);
            let memory_strings = function_args["memories"].as_array().unwrap();
            let mut memories: Vec<Memory> = Vec::new();

            for memory in memory_strings {
                memories.push(
                    Memory::add_memory(pool, memory.as_str().unwrap(), None, None, user_id, Some(memory_prompt_id), memory_cache).await?
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
    memory_cache: &Cache<String, HashMap<Uuid, Memory>>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch all memories with a given memory prompt for the user
    let user_memories = Memory::get_all_memories(&pool, user_id, memory_prompt_id, memory_cache).await?;
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
    let memory = Memory::add_memory(&app_state.pool, &req_body.content, req_body.grouping.as_deref(), req_body.emoji.as_deref(), &authenticated_user.user_id, req_body.memory_prompt_id, &app_state.memory_cache)
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
    let memories = Memory::get_all_memories(&app_state.pool, &authenticated_user.user_id, info.memory_prompt_id, &app_state.memory_cache)
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
    let memory = Memory::update_memory(&app_state.pool, memory_id.into_inner(), &req_body.content, req_body.grouping.as_deref(), req_body.emoji.as_deref(), &authenticated_user.user_id, &app_state.memory_cache)
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
    Memory::delete_memory(&app_state.pool, memory_id.into_inner(), &authenticated_user.user_id, &app_state.memory_cache)
        .await
        .map_err(|e| {
            error!("Failed to get memories: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/delete_all")]
async fn delete_all_memories(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<DeleteAllMemoriesRequest>,
) -> Result<impl Responder, actix_web::Error> {

    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized("Unauthorized".to_string()));
    }

    let user_id = req_body.user_id.clone();

    let deleted_count = Memory::delete_all_memories(&app_state.pool, &user_id, &app_state.memory_cache)
        .await
        .map_err(|e| {
            error!("Failed to delete all memories: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(HttpResponse::Ok().json(json!({ "deleted_count": deleted_count })))
}

#[post("/add_memory_prompt")]
async fn add_memory_prompt(
    app_state: web::Data<Arc<AppState>>,
    _app_config: web::Data<Arc<AppConfig>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<AddMemoryPromptRequest>,
) -> Result<impl Responder, actix_web::Error> {

    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized("Unauthorized".to_string()));
    }

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
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<GenerateMemoriesRequest>,
) -> Result<web::Json<Vec<Memory>>, actix_web::Error> {

    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized("Unauthorized".to_string()));
    }

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

    let generated_memories = process_memory_context(&app_state, &user_id, &generated_samples, req_body.memory_prompt_id).await?;

    Ok(web::Json(generated_memories))
}

// add memory_metadata
async fn process_memory_context(
    app_state: &web::Data<Arc<AppState>>,
    user_id: &str,
    samples: &Vec<String>,
    memory_prompt_id: Uuid,
) -> Result<Vec<Memory>, actix_web::Error> {
    let memory_prompt = MemoryPrompt::get_by_id(&app_state.pool, memory_prompt_id)
        .await
        .map_err(|e| {
            error!("Failed to get memory prompt: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let mut generated_memories: Vec<Memory> = Vec::new();
    // NOTE: using gpt-4o tokenizer since claude's is not open source
    let bpe = cl100k_base().unwrap();

    let futures: Vec<_> = samples.iter().enumerate().map(|(index, sample)| {
        let app_state = app_state.clone();
        let memory_prompt = memory_prompt.clone();
        let bpe = bpe.clone();

        async move {
            info!("Processing sample {} of {}", index + 1, samples.len());

            // https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering/long-context-tips
            let tokens = bpe.encode_with_special_tokens(sample);
            info!("Token count: {}", tokens.len());

            let message_content = if tokens.len() > 15000 {
                format!("{}\n\n{}\n\nReasoning for which user information to extract:\n<reasoning>\n",
                    sample,
                    memory_prompt.prompt.clone()
                )
            } else {
                format!("{}\n\n{}\n\nReasoning for which user information to extract:\n<reasoning>\n",
                    memory_prompt.prompt.clone(),
                    sample
                )
            };

            get_chat_completion(&app_state.keywords_client, "claude-3-5-sonnet-20240620", &message_content).await
        }
    }).collect();

    let results: Vec<Result<String, actix_web::Error>> = join_all(futures).await;
    for result in results {
        match result {
            Ok(result_content) => {
                let now_utc = Utc::now();
                let memory_id = Uuid::new_v4();
                let new_memory = Memory::new(memory_id, user_id, result_content.as_str(), Some(memory_prompt_id), Some(now_utc), None, None);
                generated_memories.push(new_memory);
            }
            Err(e) => error!("Error processing memory: {:?}", e),
        }
    }

    info!("Generated memories: {}", generated_memories.len());

    let formatted_memories: String = Memory::format_memories(generated_memories);

    let message_content = format!("{}\n\n{}", 
        Prompts::FORMATTING_MEMORY,
        serde_json::to_string(&formatted_memories).unwrap()
    );

    let formatted_content = get_chat_completion(&app_state.keywords_client, "claude-3-5-sonnet-20240620", &message_content).await?;

    process_formatted_memories(app_state, user_id, &formatted_content, memory_prompt_id).await
}

async fn process_formatted_memories(
    app_state: &web::Data<Arc<AppState>>,
    user_id: &str,
    formatted_content: &str,
    memory_prompt_id: Uuid,
) -> Result<Vec<Memory>, actix_web::Error> {
    let memory_regex = regex::Regex::new(r"(?s)<memory>(.*?)</memory>").unwrap();
    let mut inserted_memories = Vec::new();

    for capture in memory_regex.captures_iter(formatted_content) {
        if let Some(content) = capture.get(1) {
            let memory_content = content.as_str().trim();
            let mut lines = memory_content.lines();
            let grouping = lines.next().unwrap_or("").trim().to_string();
            let memories: Vec<String> = lines
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with('-') {
                        Some(trimmed[1..].trim().to_string())
                    } else {
                        None
                    }
                })
                .collect();
            
            let message_content = format!("{}\n\n{}", 
                Prompts::EMOJI_MEMORY,
                memory_content
            );
            let emoji_response = get_chat_completion(&app_state.keywords_client, "groq/llama3-8b-8192", &message_content).await?;
            let emoji = emoji_response.trim().chars().next().unwrap_or('📝').to_string();

            for memory in memories {
                let args = json!({
                    "memory": memory,
                    "grouping": grouping,
                    "emoji": emoji
                }).to_string();

                let new_memories = call_fn(
                    &app_state.pool,
                    "create_memory",
                    &args,
                    user_id,
                    memory_prompt_id,
                    &app_state.memory_cache,
                ).await.map_err(|e| {
                    error!("Failed to create memory: {:?}", e);
                    actix_web::error::ErrorInternalServerError(e)
                })?;

                let new_memory = new_memories.into_iter().next().unwrap_or_default();
                inserted_memories.push(new_memory);
            }
        }
    }

    Ok(inserted_memories)
}

// Add this utility function at the top of the file, after imports
async fn get_chat_completion(
    client: &Client<OpenAIConfig>,
    model: &str,
    content: &str,
) -> Result<String, actix_web::Error> {
    let ai_messages: Vec<ChatCompletionRequestMessage> = vec![
        ChatCompletionRequestUserMessageArgs::default()
            .content(content)
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                actix_web::error::ErrorInternalServerError(e)
            })?
            .into(),
    ];

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(ai_messages)
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
        })?;

    response.choices.first()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("No response from AI"))?
        .message.content.clone()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Empty response from AI"))
}
