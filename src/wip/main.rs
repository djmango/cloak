use std::collections::HashMap;
use std::error::Error;
use std::io::{stdout, Write};
use std::sync::Arc;

use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolType,
    FinishReason, FunctionCall, FunctionObjectArgs,
};
use async_openai::{config::OpenAIConfig, types::CreateChatCompletionRequestArgs, Client};
use futures::StreamExt;
use serde_json::{json, Value};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::with_config(
        OpenAIConfig::new()
            .with_api_key("api-key")
            .with_api_base("https://api.keywordsai.co/api"),
    );
    let user_prompt = "Remember I need to pick up the milk from the store.";

    let request = CreateChatCompletionRequestArgs::default()
    .max_tokens(512u32)
    .model("gpt-4o")
    .messages([ChatCompletionRequestUserMessageArgs::default()
        .content(user_prompt)
        .build()?
        .into()])
    .tools(vec![
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
                    .build()?,
            )
            .build()?,
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


    let mut stream = client.chat().create_stream(request).await?;

    let tool_call_states: Arc<Mutex<HashMap<(i32, i32), ChatCompletionMessageToolCall>>> =
        Arc::new(Mutex::new(HashMap::new()));

    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                println!("Received response: {:?}", response);
                for chat_choice in response.choices {
                    let function_responses: Arc<
                        Mutex<Vec<(ChatCompletionMessageToolCall, Value)>>,
                    > = Arc::new(Mutex::new(Vec::new()));
                    if let Some(tool_calls) = chat_choice.delta.tool_calls {
                        for (_i, tool_call_chunk) in tool_calls.into_iter().enumerate() {
                            println!("Tool call chunk: {:?}", tool_call_chunk);
                            let key = (chat_choice.index as i32, tool_call_chunk.index);
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
                    if let Some(finish_reason) = &chat_choice.finish_reason {
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
                                    println!("Calling function: {}, args: {}", name, args);
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
                            let mut messages: Vec<ChatCompletionRequestMessage> =
                                vec![ChatCompletionRequestUserMessageArgs::default()
                                    .content(user_prompt)
                                    .build()?
                                    .into()];

                            let tool_calls: Vec<ChatCompletionMessageToolCall> =
                                function_responses_lock
                                    .iter()
                                    .map(|tc| tc.0.clone())
                                    .collect();

                            let assistant_messages: ChatCompletionRequestMessage =
                                ChatCompletionRequestAssistantMessageArgs::default()
                                    .tool_calls(tool_calls)
                                    .build()
                                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                                    .unwrap()
                                    .into();

                            let tool_messages: Vec<ChatCompletionRequestMessage> =
                                function_responses_lock
                                    .iter()
                                    .map(|tc| {
                                        println!("Function response: {:?}", tc.1);
                                        ChatCompletionRequestToolMessageArgs::default()
                                            .content(tc.1.to_string())
                                            .tool_call_id(tc.0.id.clone())
                                            .build()
                                            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                                            .unwrap()
                                            .into()
                                    })
                                    .collect();

                            messages.push(assistant_messages);
                            messages.extend(tool_messages);

                            let request = CreateChatCompletionRequestArgs::default()
                                .max_tokens(512u32)
                                .model("gpt-4o")
                                .messages(messages)
                                .build()
                                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

                            let mut stream = client.chat().create_stream(request).await?;

                            let mut response_content = String::new();
                            let mut lock = stdout().lock();
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(response) => {
                                        println!("Stream response: {:?}", response);
                                        for chat_choice in response.choices.iter() {
                                            if let Some(ref content) = chat_choice.delta.content {
                                                write!(lock, "{}", content).unwrap();
                                                response_content.push_str(content);
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        return Err(Box::new(err) as Box<dyn std::error::Error>);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(content) = &chat_choice.delta.content {
                        let mut lock = stdout().lock();
                        write!(lock, "{}", content).unwrap();
                    }
                }
            }
            Err(err) => {
                let mut lock = stdout().lock();
                writeln!(lock, "error: {err}").unwrap();
            }
        }
        stdout()
            .flush()
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
    }

    Ok(())
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