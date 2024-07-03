// routes/memory.rs

use actix_web::{post, get, put, delete, web, HttpResponse, Responder};
use anyhow::Error;
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
use tracing::{info, error};
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
use chrono::{DateTime, Utc};
use tiktoken_rs::cl100k_base;
use moka::future::Cache;
use std::collections::HashMap;

use tokio::sync::Semaphore;

async fn call_fn(
    pool: &PgPool,
    name: &str,
    args: &str,
    user_id: &str,
    memory_prompt_id: Uuid,
    memory_cache: &Cache<String, HashMap<Uuid, Memory>>,
) -> Result<Vec<Memory>, Error> {
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
            let memory_strings = function_args["memories"].as_array().unwrap();
            let mut memories: Vec<Memory> = Vec::new();

            for memory in memory_strings {
                memories.push(
                    Memory::add_memory(pool, memory.as_str().unwrap(), None, None, user_id, Some(memory_prompt_id), memory_cache).await?
                );
            }

            Ok(memories)
        }
        _ => Err(Error::msg("Unknown function")),
    }
}

#[allow(dead_code)]
pub async fn get_all_user_memories(
    pool: Arc<PgPool>,
    user_id: &str,
    memory_prompt_id: Option<Uuid>,
    memory_cache: &Cache<String, HashMap<Uuid, Memory>>,
) -> Result<String, Error> {
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
pub async fn generate_memories_from_chat_history_endpoint(
    app_state: web::Data<Arc<AppState>>,
    _app_config: web::Data<Arc<AppConfig>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<GenerateMemoriesRequest>,
) -> Result<web::Json<Vec<Memory>>, actix_web::Error> {

    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized("Unauthorized".to_string()));
    }

    let user_id = req_body.user_id.clone();
    let memory_prompt_id = req_body.memory_prompt_id.clone();
    let range = req_body.range.map(|(start, end)| {
        (
            DateTime::<Utc>::from_timestamp(start as i64, 0).unwrap(),
            DateTime::<Utc>::from_timestamp(end as i64, 0).unwrap()
        )
    });

    generate_memories_from_chat_history(
        &app_state, 
        None,
        &user_id, 
        &memory_prompt_id, 
        req_body.max_samples, 
        req_body.samples_per_query, 
        range
    ).await.map_err(|e| actix_web::error::ErrorInternalServerError(e))
}

pub async fn generate_memories_from_chat_history(
    app_state: &web::Data<Arc<AppState>>,
    sem: Option<Arc<Semaphore>>,
    user_id: &str,
    memory_prompt_id: &Uuid,
    max_samples: Option<u32>,
    samples_per_query: Option<u32>,
    range: Option<(DateTime<Utc>, DateTime<Utc>)>,
) -> Result<web::Json<Vec<Memory>>, Error> {

    // get most recent message
    let latest_msg = Message::get_latest_message_by_user_id(&app_state.pool, &user_id).await?;

    // skip users who haven't sent message in last 14 days
    if let Some(latest_msg) = latest_msg {
        if let Some(_) = range {
            if latest_msg.created_at < Utc::now() - chrono::Duration::days(13) {
                return Err(anyhow::anyhow!("Invalid User: User does not meet requirements for generating memory"));
            }
        }
    }

    let mut user_messages = Message::get_messages_by_user_id(&app_state.pool, &user_id, range).await?;

    // skip users with no messages to generate memory for
    if user_messages.is_empty() {
        return Err(anyhow::anyhow!("Invalid User: User does not meet requirements for generating memory"));
    }

    let max_samples = match max_samples {
        Some(n) => n,
        None => user_messages.len() as u32
    };

    let samples_per_query = match samples_per_query {
        Some(n) => n,
        None => 30
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

        start_index += samples_per_query as usize;
    }
        
    info!("Generated {} samples", generated_samples.len());

    let generated_memories = process_memory_context(&app_state, sem, &user_id, &generated_samples, memory_prompt_id.clone()).await?;

    Ok(web::Json(generated_memories))
}

// add memory_metadata
async fn process_memory_context(
    app_state: &web::Data<Arc<AppState>>,
    sem: Option<Arc<Semaphore>>,
    user_id: &str,
    samples: &Vec<String>,
    memory_prompt_id: Uuid,
) -> Result<Vec<Memory>, Error> {
    let memory_prompt = MemoryPrompt::get_by_id(&app_state.pool, memory_prompt_id)
        .await
        .map_err(|e| {
            error!("Failed to get memory prompt: {:?}", e);
            e
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

    let results: Vec<Result<String, Error>> = join_all(futures).await;
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

    let formatted_memories = process_formatted_memories( user_id, &formatted_content, memory_prompt_id).await?;

    let existing_memories = Memory::get_all_memories(&app_state.pool, user_id, None, &app_state.memory_cache).await?;

    increment_memory(app_state, sem, &formatted_memories, &existing_memories).await
}

async fn process_formatted_memories(
    user_id: &str,
    formatted_content: &str,
    memory_prompt_id: Uuid,
) -> Result<Vec<Memory>, Error> {
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

            inserted_memories.extend(memories.into_iter().map(|memory| Memory::new(
                Uuid::new_v4(),
                user_id,
                &memory,
                Some(memory_prompt_id),
                None,
                Some(&grouping),
                None
            )));
        }
    }

    Ok(inserted_memories)
}

async fn increment_memory(
    app_state: &web::Data<Arc<AppState>>,
    sem: Option<Arc<Semaphore>>,
    new_memories: &Vec<Memory>,
    existing_memories: &Vec<Memory>,
) -> Result<Vec<Memory>, Error> {
    // Format memories
    let formatted_memories = Memory::format_grouped_memories(existing_memories);
    let prompt = format!(
        r#"Analyze the following new memories and determine if each belongs to an existing memory grouping, requires a new one, or is redundant.

New Memories:
{}

Existing Memory Groupings:
{}

For each new memory, provide your analysis in the following format:
<filtered memory>
Content: {{memory content}}
Reasoning: {{Your step-by-step reasoning here}}
Verdict: NEW, {{new_grouping_name}} || OLD, {{existing_grouping_name}} || REPEAT
</filtered memory>

Rules:
1. If the memory fits into an existing grouping, use "OLD" verdict with the existing grouping name.
2. If the memory requires a new grouping, use "NEW" verdict with a suggested grouping name. Grouping name should be no more than 2 words, and should be simple, friendly, and human-readable.
3. If the memory is redundant or too similar to existing memories, use "REPEAT" verdict.
4. Provide clear reasoning for each decision.
5. Each content, reasoning, and verdict should be a single line. There should be only one newline that separates each, no more. 
"#,
        new_memories.iter().map(|m| m.content.clone()).collect::<Vec<_>>().join("\n\n"),
        formatted_memories
    );

    info!("Prompt:\n {}", prompt);

    let response = get_chat_completion(&app_state.keywords_client, "claude-3-5-sonnet-20240620", &prompt).await?;

    info!("AI Response:\n {}", response);

    let memory_regex: regex::Regex = regex::Regex::new(r"(?m)^Content: (.*)$\n^Reasoning:.*$\n^Verdict: (.*)$").unwrap();
    let mut filtered_memories = Vec::new();

    for (idx, capture) in memory_regex.captures_iter(&response).enumerate() {
        let content = capture.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        let verdict = capture.get(3).map(|m| m.as_str().trim()).unwrap_or("");

        info!("Memory {}:\n Content: {},\n Verdict: {}", idx, content, verdict);

        let (verdict_type, grouping) = match verdict.split_once(',') {
            Some((v, g)) => (v.trim(), g.trim()),
            None => continue,
        };

        match verdict_type {
            "NEW" | "OLD" => {
                if let Some(memory) = new_memories.iter().find(|m| m.content == content) {
                    let message_content = format!("{}\n\n{}", 
                        Prompts::EMOJI_MEMORY,
                        grouping
                    );

                    let emoji_response = get_chat_completion(&app_state.keywords_client, "groq/llama3-70b-8192", &message_content).await?;
                    let emoji = emoji_response.trim().chars().next().unwrap_or('📝').to_string();

                    let mut new_memory = memory.clone();
                    new_memory.grouping = Some(grouping.to_string());
                    new_memory.emoji = Some(emoji);
                    filtered_memories.push(new_memory);
                }
            }
            "REPEAT" => {
                continue;
            }
            _ => {
                error!("Invalid verdict type: {}", verdict_type);
                continue;
            }
        }
    }

    let mut added_memories = Vec::new();
    for memory in filtered_memories {
        let args = json!({
            "memory": memory.content,
            "grouping": memory.grouping,
            "emoji": memory.emoji
        }).to_string();

        // Acquire the semaphore permit if provided
        let _permit = if let Some(sem) = &sem {
            Some(sem.acquire().await?)
        } else {
            None
        };

        let new_memories = call_fn(
            &app_state.pool,
            "create_memory",
            &args,
            &memory.user_id,
            memory.memory_prompt_id.unwrap_or_default(),
            &app_state.memory_cache,
        ).await.map_err(|e| {
            error!("Failed to create memory: {:?}", e);
            e
        })?;

        // The semaphore permit is automatically released here when _permit goes out of scope
        if let Some(new_memory) = new_memories.into_iter().next() {
            added_memories.push(new_memory);
        }
    }

    Ok(added_memories)
}
// Add this utility function at the top of the file, after imports
async fn get_chat_completion(
    client: &Client<OpenAIConfig>,
    model: &str,
    content: &str,
) -> Result<String, Error> {
    let ai_messages: Vec<ChatCompletionRequestMessage> = vec![
        ChatCompletionRequestUserMessageArgs::default()
            .content(content)
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                e
            })?
            .into(),
    ];

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(ai_messages)
        .build()
        .map_err(|e| {
            error!("Failed to build chat completion request: {:?}", e);
            e
        })?;

    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| {
            error!("Failed to get chat completion response: {:?}", e);
            e
        })?;

    response.choices.first()
        .ok_or_else(|| Error::msg("No response from AI"))?
        .message.content.clone()
        .ok_or_else(|| Error::msg("Empty response from AI"))
}

