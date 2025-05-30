// routes/memory.rs

use actix_web::{delete, get, post, put, web, HttpResponse};
use anyhow::{Context, Error, Result};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::Client;
use chrono::{DateTime, Utc};
use futures::future::{join_all, try_join_all};
use lazy_static::lazy_static;
use moka::future::Cache;
use regex::Regex;
use serde_json::json;
use sqlx::PgPool;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tiktoken_rs::cl100k_base;
use tokio::sync::Semaphore;
use tracing::{error, info};
use uuid::Uuid;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::{Memory, Message};
use crate::prompts::Prompts;
use crate::types::{CreateMemoryRequest, GenerateMemoriesRequest, UpdateMemoryRequest};
use crate::AppConfig;
use crate::AppState;

async fn call_fn(
    pool: &PgPool,
    name: &str,
    args: &str,
    user_id: &str,
    memory_cache: &Cache<String, HashMap<Uuid, Memory>>,
) -> Result<Vec<Memory>> {
    let function_args: serde_json::Value = args.parse()?;

    match name {
        "create_memory" => {
            let memory = function_args["memory"].as_str().unwrap();
            let grouping = function_args.get("grouping").and_then(|g| g.as_str());
            let new_memory =
                Memory::add_memory(pool, memory, grouping, user_id, memory_cache).await?;
            Ok(vec![new_memory])
        }
        "update_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let new_memory = function_args["memory"].as_str().unwrap();
            let grouping = function_args.get("grouping").and_then(|g| g.as_str());
            let updated_memory =
                Memory::update_memory(pool, memory_id, new_memory, grouping, user_id, memory_cache)
                    .await?;

            Ok(vec![updated_memory])
        }
        "delete_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let deleted_memory =
                Memory::delete_memory(pool, memory_id, user_id, memory_cache).await?;

            Ok(vec![deleted_memory])
        }
        "parse_memories" => {
            let memory_strings = function_args["memories"].as_array().unwrap();
            let mut memories: Vec<Memory> = Vec::new();

            for memory in memory_strings {
                memories.push(
                    Memory::add_memory(pool, memory.as_str().unwrap(), None, user_id, memory_cache)
                        .await?,
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
    memory_cache: &Cache<String, HashMap<Uuid, Memory>>,
) -> Result<String> {
    // Fetch all memories with a given memory prompt for the user
    let user_memories = Memory::get_all_memories(&pool, user_id, memory_cache).await?;
    let formatted_memories = Memory::format_memories(user_memories);

    Ok(formatted_memories)
}

// create
#[post("/create")]
async fn create_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<CreateMemoryRequest>,
) -> Result<web::Json<Memory>, actix_web::Error> {
    let memory = Memory::add_memory(
        &app_state.pool,
        &req_body.content,
        req_body.grouping.as_deref(),
        &authenticated_user.user_id,
        &app_state.memory_cache,
    )
    .await
    .map_err(|e| {
        error!("Failed to create memory: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(web::Json(memory))
}

// read
#[get("/")]
async fn get_memories(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
) -> Result<web::Json<Vec<Memory>>, actix_web::Error> {
    let memories = Memory::get_all_memories(
        &app_state.pool,
        &authenticated_user.user_id,
        &app_state.memory_cache,
    )
    .await
    .map_err(|e| {
        error!("Failed to get memories: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(web::Json(memories))
}

// update
#[put("/{memory_id}")]
async fn update_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    memory_id: web::Path<Uuid>,
    req_body: web::Json<UpdateMemoryRequest>,
) -> Result<web::Json<Memory>, actix_web::Error> {
    let memory = Memory::update_memory(
        &app_state.pool,
        memory_id.into_inner(),
        &req_body.content,
        req_body.grouping.as_deref(),
        &authenticated_user.user_id,
        &app_state.memory_cache,
    )
    .await
    .map_err(|e| {
        error!("Failed to get memories: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(web::Json(memory))
}

// delete
#[delete("/{memory_id}")]
async fn delete_memory(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    memory_id: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    Memory::delete_memory(
        &app_state.pool,
        memory_id.into_inner(),
        &authenticated_user.user_id,
        &app_state.memory_cache,
    )
    .await
    .map_err(|e| {
        error!("Failed to get memories: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/generate_from_chat")]
pub async fn generate_memories_from_chat_history_endpoint(
    app_state: web::Data<Arc<AppState>>,
    _app_config: web::Data<Arc<AppConfig>>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<GenerateMemoriesRequest>,
) -> Result<web::Json<Vec<Memory>>, actix_web::Error> {
    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized(
            "Unauthorized".to_string(),
        ));
    }

    let user_id = req_body.user_id.clone();
    let range = req_body.range.map(|(start, end)| {
        (
            DateTime::<Utc>::from_timestamp(start as i64, 0).unwrap(),
            DateTime::<Utc>::from_timestamp(end as i64, 0).unwrap(),
        )
    });

    generate_memories_from_chat_history(
        &app_state,
        None,
        &user_id,
        req_body.max_samples,
        req_body.samples_per_query,
        range,
    )
    .await
    .map(web::Json)
    .map_err(|e| {
        error!("Failed to generate memories: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })
}

lazy_static! {
    static ref max_sample_toks: usize = 50000;
    static ref long_ctxt_tokens: usize = 15000;
}

pub async fn generate_memories_from_chat_history(
    app_state: &web::Data<Arc<AppState>>,
    sem: Option<Arc<Semaphore>>,
    user_id: &str,
    max_samples: Option<u32>,
    samples_per_query: Option<u32>,
    range: Option<(DateTime<Utc>, DateTime<Utc>)>,
) -> Result<Vec<Memory>> {
    // get most recent message
    let latest_msg = Message::get_latest_message_by_user_id(&app_state.pool, user_id).await?;

    // skip users who haven't sent message in last 14 days
    if range.is_some() {
        match latest_msg {
            Some(msg) if msg.created_at < Utc::now() - chrono::Duration::days(13) => {
                return Err(anyhow::anyhow!(
                    "Invalid User: User has not sent a message in the last 13 days, which is required for memory generation"
                ));
            }
            None => {
                return Err(anyhow::anyhow!(
                    "Invalid User: No messages found for this user"
                ));
            }
            _ => {} // User has a message within the last 13 days
        }
    }

    let mut user_messages =
        Message::get_messages_by_user_id(&app_state.pool, user_id, range).await?;

    // Skip users with no messages to generate memory for
    if user_messages.is_empty() {
        return Err(anyhow::anyhow!(
            "Invalid User: No messages found for this user. At least one message is required to generate memories."
        ));
    }

    let max_samples = match max_samples {
        Some(n) => n,
        None => user_messages.len() as u32,
    };

    let samples_per_query = match samples_per_query {
        Some(n) => n,
        None => user_messages.len() as u32,
    };

    if user_messages.len() as u32 > max_samples {
        user_messages = user_messages
            .into_iter()
            .take(max_samples as usize)
            .collect();
    }

    user_messages.reverse();

    let estimate_token_count = |text: &str| text.chars().count() / 4;
    let bpe = cl100k_base().context("Failed to initialize tokenizer");
    let mut msgs_queue: VecDeque<Message> = VecDeque::from(user_messages.clone());
    let mut generated_samples = Vec::new();
    let mut memory_content = String::new();
    let mut i = 0;
    let mut sample_tokens = 0;
    let mut working_tokens: Option<(Uuid, Vec<usize>)> = None;

    info!(
        "Begin processing samples of max {} tokens",
        *max_sample_toks
    );

    while !msgs_queue.is_empty() {
        if i >= samples_per_query {
            i = 0;
            info!(
                "Sample {}: Total token count: {}",
                generated_samples.len() + 1,
                sample_tokens
            );
            memory_content += "</end chat>\n";
            sample_tokens = 0;
            generated_samples.push(memory_content.clone());
            memory_content = String::new();
        }

        if let Some(msg) = msgs_queue.pop_front() {
            i += 1;
            let message_content = format!(
                "<message {}: {}>\n{}\n</message {}: {}>\n",
                i, msg.role, msg.text, i, msg.role
            );

            let num_tokens = if let Some((cached_msg_id, cached_tokens)) = &working_tokens {
                if cached_msg_id == &msg.id {
                    cached_tokens.len()
                } else {
                    match &bpe {
                        Ok(tokenizer) => {
                            let tokens = tokenizer.encode_with_special_tokens(&msg.text);
                            working_tokens = Some((msg.id, tokens.clone()));
                            tokens.len()
                        }
                        Err(_) => estimate_token_count(&msg.text),
                    }
                }
            } else {
                match &bpe {
                    Ok(tokenizer) => {
                        let tokens = tokenizer.encode_with_special_tokens(&msg.text);
                        working_tokens = Some((msg.id, tokens.clone()));
                        tokens.len()
                    }
                    Err(_) => estimate_token_count(&msg.text),
                }
            };

            sample_tokens += num_tokens;
            if sample_tokens > *max_sample_toks {
                let remaining_tokens = *max_sample_toks - (sample_tokens - num_tokens);
                let truncated_msg = if let Ok(tokenizer) = &bpe {
                    let truncated_tokens = working_tokens
                        .as_ref()
                        .map(|(_, tokens)| tokens[..remaining_tokens].to_vec())
                        .unwrap_or_else(|| {
                            tokenizer.encode_with_special_tokens(&msg.text)[..remaining_tokens]
                                .to_vec()
                        });
                    tokenizer
                        .decode(truncated_tokens)
                        .context("Failed to decode truncated message")
                        .unwrap_or_else(|_| msg.text.chars().take(remaining_tokens * 4).collect())
                } else {
                    msg.text.chars().take(remaining_tokens * 4).collect()
                };

                let truncated_content = format!(
                    "<message {}: {}>\n{}\n</message {}: {}>\n",
                    i, msg.role, truncated_msg, i, msg.role
                );

                i = 0;
                info!(
                    "Sample {}: Total token count: {}",
                    generated_samples.len() + 1,
                    *max_sample_toks
                );
                sample_tokens = 0;
                memory_content.push_str(&truncated_content);
                generated_samples.push(memory_content.clone());
                memory_content = String::new();

                let remaining_msg = if let Ok(tokenizer) = &bpe {
                    let remaining_tokens = working_tokens
                        .as_ref()
                        .map(|(_, tokens)| tokens[remaining_tokens..].to_vec())
                        .unwrap_or_else(|| {
                            tokenizer.encode_with_special_tokens(&msg.text)[remaining_tokens..]
                                .to_vec()
                        });
                    let remaining_len = remaining_tokens.len();
                    working_tokens = Some((msg.id, remaining_tokens.clone()));
                    tokenizer
                        .decode(remaining_tokens)
                        .context("Failed to decode remaining message")
                        .unwrap_or_else(|_| msg.text.chars().skip(remaining_len * 4).collect())
                } else {
                    msg.text.chars().skip(remaining_tokens * 4).collect()
                };
                msgs_queue.push_front(Message {
                    text: remaining_msg,
                    ..msg
                });
            } else {
                memory_content.push_str(&message_content);
            }
        }
    }
    if !memory_content.is_empty() {
        info!(
            "Sample {}: Total token count: {}",
            generated_samples.len() + 1,
            sample_tokens
        );
        generated_samples.push(memory_content);
    }

    info!("Generated {} samples", generated_samples.len());

    let generated_memories =
        process_memory_context(app_state, sem, user_id, &generated_samples).await?;

    Ok(generated_memories)
}

// add memory_metadata
async fn process_memory_context(
    app_state: &web::Data<Arc<AppState>>,
    sem: Option<Arc<Semaphore>>,
    user_id: &str,
    samples: &[String],
) -> Result<Vec<Memory>> {
    let mut generated_memories: Vec<Memory> = Vec::new();
    // NOTE: using gpt-4o tokenizer since claude's is not open source
    let bpe = cl100k_base().unwrap();

    let futures: Vec<_> = samples.iter().enumerate().map(|(index, sample)| {
        let app_state = app_state.clone();
        let bpe = bpe.clone();

        async move {
            info!("Processing sample {} of {}", index + 1, samples.len());

            // https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering/long-context-tips
            let tokens = bpe.encode_with_special_tokens(sample);
            info!("Token count: {}", tokens.len());

            let message_content = if tokens.len() > 15000 {
                format!("{}\n\n{}\n\nReasoning for which user information to extract:\n<reasoning>\n",
                    sample,
                    Prompts::GENERATE_MEMORY
                )
            } else {
                format!("{}\n\n{}\n\nReasoning for which user information to extract:\n<reasoning>\n",
                    Prompts::GENERATE_MEMORY,
                    sample
                )
            };

            get_chat_completion(&app_state.keywords_client, "gemini/gemini-1.5-flash", &message_content).await
        }
    }).collect();

    let results: Vec<Result<String>> = join_all(futures).await;
    for result in results {
        match result {
            Ok(result_content) => {
                let now_utc = Utc::now();
                let memory_id = Uuid::new_v4();
                let new_memory = Memory::new(
                    memory_id,
                    user_id,
                    result_content.as_str(),
                    Some(now_utc),
                    None,
                );
                generated_memories.push(new_memory);
            }
            Err(e) => error!("Error processing memory: {:?}", e),
        }
    }

    info!("Generated memories: {}", generated_memories.len());

    let formatted_memories: String = Memory::format_memories(generated_memories);

    let format_memory_prompt = Prompts::FORMATTING_MEMORY
        .replace("{0}", &Memory::formated_allowed_groups())
        .replace("{1}", &serde_json::to_string(&formatted_memories).unwrap());

    let formatted_content = get_chat_completion(
        &app_state.keywords_client,
        "gemini/gemini-1.5-flash",
        &format_memory_prompt,
    )
    .await?;

    let formatted_memories = process_formatted_memories(user_id, &formatted_content).await?;

    info!("formatted memories:");
    for memory in &formatted_memories {
        info!(
            "ID: {}, Content: {}, Grouping: {:?}",
            memory.id, memory.content, memory.grouping
        );
    }

    let existing_memories =
        Memory::get_all_memories(&app_state.pool, user_id, &app_state.memory_cache).await?;

    increment_memory(
        app_state,
        user_id,
        sem,
        &formatted_memories,
        &existing_memories,
    )
    .await
}

async fn process_formatted_memories(user_id: &str, formatted_content: &str) -> Result<Vec<Memory>> {
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
                    trimmed.strip_prefix('-').map(|s| s.trim().to_string())
                })
                .collect();

            inserted_memories.extend(memories.into_iter().map(|memory| {
                Memory::new(Uuid::new_v4(), user_id, &memory, None, Some(&grouping))
            }));
        }
    }

    Ok(inserted_memories)
}

async fn increment_memory(
    app_state: &web::Data<Arc<AppState>>,
    user_id: &str,
    sem: Option<Arc<Semaphore>>,
    new_memories: &[Memory],
    existing_memories: &Vec<Memory>,
) -> Result<Vec<Memory>> {
    let new_memory_count = new_memories.len();
    let existing_memory_count = existing_memories.len();

    info!("Memory Stats:");
    info!("  • New memories:     {:>5}", new_memory_count);
    info!("  • Existing memories:{:>5}", existing_memory_count);

    if existing_memories.is_empty() {
        let added_memories =
            process_memories(app_state, "create_memory", user_id, sem, new_memories).await?;
        let added_memory_count = added_memories.len();
        let new_total_count = added_memory_count;

        info!("  • Added memories:   {:>5}", added_memory_count);
        info!("  • New total count:  {:>5}", new_total_count);

        return Ok(added_memories);
    }

    let format_with_id = true;
    let formatted_memories = Memory::format_grouped_memories(existing_memories, format_with_id);
    let allowed_groups_list_str = Memory::formated_allowed_groups();
    let new_memories_str = new_memories
        .iter()
        .map(|m| format!("- {}", m.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    let prompt = Prompts::INCREMENT_MEMORY
        .replace("{0}", &formatted_memories)
        .replace("{1}", &new_memories_str)
        .replace("{2}", &allowed_groups_list_str);

    let response = get_chat_completion(
        &app_state.keywords_client,
        "gemini/gemini-1.5-flash",
        &prompt,
    )
    .await?;
    let filtered_memories =
        parse_ai_response(app_state, user_id, existing_memories, &response).await?;

    let memories_to_update: Vec<Memory> = filtered_memories
        .iter()
        .filter(|memory| existing_memories.iter().any(|m| m.id == memory.id))
        .cloned()
        .collect();

    let memories_to_add: Vec<Memory> = filtered_memories
        .iter()
        .filter(|memory| !memories_to_update.iter().any(|m| m.id == memory.id))
        .cloned()
        .collect();

    let updated_memories = process_memories(
        app_state,
        "update_memory",
        user_id,
        sem.clone(),
        &memories_to_update,
    )
    .await?;
    let added_memories =
        process_memories(app_state, "create_memory", user_id, sem, &memories_to_add).await?;
    let updated_memory_count = updated_memories.len();
    let added_memory_count = added_memories.len();
    let new_total_count = existing_memory_count + added_memory_count;

    info!("  • Updated memories: {:>5}", updated_memory_count);
    info!("  • Added memories:   {:>5}", added_memory_count);
    info!("  • New total count:  {:>5}", new_total_count);

    Ok([updated_memories, added_memories].concat())
}

async fn process_memories(
    app_state: &web::Data<Arc<AppState>>,
    op_type: &str,
    user_id: &str,
    sem: Option<Arc<Semaphore>>,
    memories: &[Memory],
) -> Result<Vec<Memory>> {
    let futures = memories.iter().map(|memory| {
        let app_state = app_state.clone();
        let sem = sem.clone();
        async move {
            let args = json!({
                "memory_id": memory.id,
                "memory": memory.content,
                "grouping": memory.grouping,
            })
            .to_string();

            let _permit = match sem.as_ref() {
                Some(s) => Some(s.acquire().await?),
                None => None,
            };

            call_fn(
                &app_state.pool,
                op_type,
                &args,
                user_id,
                &app_state.memory_cache,
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Error::msg("Failed to create memory"))
        }
    });

    try_join_all(futures).await
}

lazy_static! {
    static ref FILTERED_MEMORY_REGEX: Regex =
        Regex::new(r"(?s)<filtered memory>(.*?)</filtered memory>").unwrap();
    static ref CONTENT_REGEX: Regex = Regex::new(r"Content:\s*(.+)").unwrap();
    static ref VERDICT_REGEX: Regex = Regex::new(r"Verdict:\s*(.+)").unwrap();
    static ref UPDATED_MEMORY_REGEX: Regex =
        Regex::new(r"(?s)<updated memory>(.*?)</updated memory>").unwrap();
    static ref CLASSIFY_MESSAGE_REGEX: Regex =
        Regex::new(r"(?is)<classification>\s*(.*?)\s*</classification>").unwrap();
}

async fn parse_ai_response(
    app_state: &web::Data<Arc<AppState>>,
    user_id: &str,
    existing_memories: &[Memory],
    response: &str,
) -> Result<Vec<Memory>, Error> {
    let futures = FILTERED_MEMORY_REGEX
        .captures_iter(response)
        .enumerate()
        .map(|(idx, capture)| async move {
            let memory_block = match capture.get(1) {
                Some(m) => m.as_str().trim(),
                None => return None,
            };

            let content = match CONTENT_REGEX
                .captures(memory_block)
                .and_then(|cap| cap.get(1))
            {
                Some(m) => m.as_str().trim(),
                None => return None,
            };

            let verdict = match VERDICT_REGEX
                .captures(memory_block)
                .and_then(|cap| cap.get(1))
            {
                Some(m) => m.as_str().trim(),
                None => return None,
            };

            let (verdict_type, grouping_or_memory) = match verdict.split_once(',') {
                Some((v, g)) => (v.trim(), Some(g.trim())),
                None if verdict == "REPEAT" => ("REPEAT", None),
                _ => return None,
            };

            info!(
                "Memory {}:\n Content: {},\n Verdict: {}",
                idx, content, verdict_type
            );

            match verdict_type {
                "NEW" | "OLD" => {
                    let grouping = Memory::get_valid_group(grouping_or_memory);
                    Some(Memory::new(
                        Uuid::new_v4(),
                        user_id,
                        content,
                        None,
                        Some(&grouping),
                    ))
                }
                "UPDATE" => {
                    let memory_id = match grouping_or_memory.and_then(|id| Uuid::parse_str(id).ok())
                    {
                        Some(id) => id,
                        None => return None,
                    };

                    info!("Memory ID to update: {}", memory_id);

                    let existing_memory = match existing_memories.iter().find(|m| m.id == memory_id)
                    {
                        Some(memory) => memory,
                        None => return None,
                    };

                    let message_content = format!(
                        "{}\n\nOLD MEMORY:\n{}\nNEW MEMORY:\n{}",
                        Prompts::UPDATE_MEMORY,
                        existing_memory.content,
                        content
                    );

                    info!("Update prompt:\n{}", message_content);

                    let updated_content = match get_chat_completion(
                        &app_state.keywords_client,
                        "gemini/gemini-1.5-flash",
                        &message_content,
                    )
                    .await
                    {
                        Ok(content) => {
                            info!("Chat completion result: {}", content);
                            content
                        }
                        Err(e) => {
                            info!("Failed to get chat completion at idx {}: {:?}", idx, e);
                            return None;
                        }
                    };

                    let updated_memory = match UPDATED_MEMORY_REGEX
                        .captures(&updated_content)
                        .and_then(|updated_cap| updated_cap.get(1))
                        .map(|updated_memory| {
                            Memory::new(
                                existing_memory.id,
                                user_id,
                                updated_memory.as_str().trim(),
                                None,
                                existing_memory.grouping.as_deref(),
                            )
                        }) {
                        Some(memory) => memory,
                        None => return None,
                    };

                    info!("Updated memory: {:?}", updated_memory);
                    Some(updated_memory)
                }
                "REPEAT" => None,
                _ => None,
            }
        });

    let results = futures::future::join_all(futures).await;
    Ok(results.into_iter().flatten().collect())
}

pub async fn use_message_for_memory(
    app_state: &web::Data<Arc<AppState>>,
    message_content: &str,
) -> Result<bool, Error> {
    let classify_prompt = Prompts::CLASSIFY_INSTRUCTION.replace("{0}", message_content);
    info!("classify_prompt:\n{}", classify_prompt);
    match get_chat_completion(
        &app_state.keywords_client,
        "groq/llama3-70b-8192",
        &classify_prompt,
    )
    .await
    {
        Ok(res) => {
            info!("AI output:\n{}", res);
            let captures = CLASSIFY_MESSAGE_REGEX.captures(&res);
            info!("Regex captures: {:?}", captures);
            let result = captures.and_then(|cap| cap.get(1)).map_or(false, |m| {
                let matched = m.as_str().trim().to_uppercase();
                info!(
                    "Matched text: '{}', Uppercase: '{}'",
                    m.as_str().trim(),
                    matched
                );
                matched == "REMEMBER"
            });
            info!("Classification result: {}", result);
            Ok(result)
        }
        Err(e) => {
            error!("Failed to classify message: {}", e);
            Err(e)
        }
    }
}
// Add this utility function at the top of the file, after imports
async fn get_chat_completion(
    client: &Client<OpenAIConfig>,
    model: &str,
    content: &str,
) -> Result<String, Error> {
    let ai_messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestUserMessageArgs::default()
            .content(content)
            .build()
            .map_err(|e| {
                error!("Failed to build user message: {:?}", e);
                e
            })?
            .into()];

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(ai_messages)
        .build()
        .map_err(|e| {
            error!("Failed to build chat completion request: {:?}", e);
            e
        })?;

    let response = client.chat().create(request).await.map_err(|e| {
        error!("Failed to get chat completion response: {:?}", e);
        e
    })?;

    response
        .choices
        .first()
        .ok_or_else(|| Error::msg("No response from AI"))?
        .message
        .content
        .clone()
        .ok_or_else(|| Error::msg("Empty response from AI"))
}
