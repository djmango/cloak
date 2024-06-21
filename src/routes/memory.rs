// routes/memory.rs

use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionToolArgs, 
    ChatCompletionToolType, CreateChatCompletionRequestArgs,
    FunctionObjectArgs,
};
use async_openai::Client;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::models::memory::Memory;

pub async fn process_memory(
    pool: Arc<PgPool>,
    user_id: String,
    messages: Vec<ChatCompletionRequestMessage>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();

    // Fetch all memories for the user
    let user_memories = Memory::get_all_memories(&pool, &user_id).await?;
    let formatted_memories = Memory::format_memories(user_memories);

    // Prepare the messages for the AI, including the formatted memories
    let mut ai_messages = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content(format!(
                "You are an AI assistant with access to the following user memories:\n{}",
                formatted_memories
            ))
            .build()?
            .into(),
    ];
    ai_messages.extend(messages);

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u32)
        .model("gpt-4-1106-preview")
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
        .get(0)
        .unwrap()
        .message
        .clone();

    if let Some(tool_calls) = response_message.tool_calls {
        for tool_call in tool_calls {
            let name = tool_call.function.name.clone();
            let args = tool_call.function.arguments.clone();

            call_fn(&pool, &name, &args, &user_id).await?;
        }
    }

    let final_request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u32)
        .model("gpt-4-1106-preview")
        .messages(vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content("Summarize the interaction and any changes to the user's memories.")
                .build()?
                .into(),
            ChatCompletionRequestAssistantMessageArgs::default()
                .content(response_message.content.unwrap_or_default())
                .build()?
                .into(),
        ])
        .build()?;

    let final_response = client.chat().create(final_request).await?;
    let response_content = final_response.choices[0].message.content.clone().unwrap_or_default();

    Ok(response_content)
}

async fn call_fn(
    pool: &PgPool,
    name: &str,
    args: &str,
    user_id: &str,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let function_args: serde_json::Value = args.parse()?;

    match name {
        "create_memory" => {
            let memory = function_args["memory"].as_str().unwrap();
            let new_memory = Memory::add_memory(pool, memory, user_id).await?;
            Ok(json!({
                "status": "success",
                "memory_id": new_memory.id,
                "message": "Memory created successfully."
            }))
        }
        "update_memory" => {
            let memory_id = Uuid::parse_str(function_args["memory_id"].as_str().unwrap())?;
            let new_memory = function_args["new_memory"].as_str().unwrap();
            let updated_memory = Memory::update_memory(pool, memory_id, new_memory, user_id).await?;
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
        _ => Err("Unknown function".into()),
    }
}