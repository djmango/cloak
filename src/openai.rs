use actix_web::{get, post, web, HttpResponse, Responder};
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs,
};
use tracing::info;

use crate::AppState;

#[get("/ai")]
async fn ai(state: web::Data<AppState>) -> Result<impl Responder, actix_web::Error> {
    info!("AI endpoint hit");

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u16)
        .model("gpt-3.5-turbo")
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content("You are a helpful assistant.")
                .build()
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content("Who won the world series in 2020?")
                .build()
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
                .into(),
            ChatCompletionRequestAssistantMessageArgs::default()
                .content("The Los Angeles Dodgers won the World Series in 2020.")
                .build()
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content("Where was it played? And the next one")
                .build()
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
                .into(),
        ])
        .build()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Call API
    let response = state
        .oai_client
        .chat()
        .create(request)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    let content = response
        .choices
        .first()
        .ok_or_else(|| {
            actix_web::error::ErrorInternalServerError("No response from OpenAI".to_string())
        })?
        .message
        .content
        .as_ref()
        .ok_or_else(|| {
            actix_web::error::ErrorInternalServerError(
                "No content in response from OpenAI".to_string(),
            )
        })?
        .clone();

    info!("{}", &content);

    Ok(HttpResponse::Ok().content_type("text/plain").body(content))
}

#[post("/v1/chat/completions")]
async fn chat(
    state: web::Data<AppState>,
    req_body: web::Json<CreateChatCompletionRequest>,
) -> Result<impl Responder, actix_web::Error> {
    info!("AI endpoint hit with model: {}", req_body.model);

    let request_args = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u16)
        .model(&req_body.model)
        .messages(
            req_body
                .messages
                .iter()
                .map(|msg| {
                    match msg {
                        ChatCompletionRequestMessage::System(system_msg) => {
                            // Here, system_msg is of type ChatCompletionRequestSystemMessage
                            // You can now access its fields or methods.
                            // For example, assuming .content() returns a content String or &str
                            ChatCompletionRequestSystemMessageArgs::default()
                                .content(&system_msg.content)
                                .build()
                                .expect("Valid system message")
                                .into()

                            // Placeholder for actual handling logic
                        }
                        ChatCompletionRequestMessage::User(user_msg) => {
                            // Assuming "user" role, handle other roles as needed
                            ChatCompletionRequestUserMessageArgs::default()
                                .content(user_msg.content.clone())
                                .build()
                                .expect("Valid user message")
                                .into()
                        }
                        ChatCompletionRequestMessage::Assistant(assistant_msg) => {
                            // And so forth for each variant
                            ChatCompletionRequestAssistantMessageArgs::default()
                                .content(
                                    assistant_msg
                                        .content
                                        .clone()
                                        .expect("Valid assistant message"),
                                )
                                .build()
                                .expect("Valid assistant message")
                                .into()
                        }
                        ChatCompletionRequestMessage::Tool(tool_msg) => {
                            ChatCompletionRequestToolMessageArgs::default()
                                .content(tool_msg.content.clone())
                                .build()
                                .expect("Valid system message")
                                .into()
                        }
                        ChatCompletionRequestMessage::Function(function_msg) => {
                            ChatCompletionRequestToolMessageArgs::default()
                                .content(
                                    function_msg.content.clone().expect("Valid system message"),
                                )
                                .build()
                                .expect("Valid system message")
                                .into()
                        }
                    }
                })
                .collect::<Vec<_>>(),
        )
        .build()
        .expect("Valid request");

    let response = state
        .oai_client
        .chat()
        .create(request_args)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Assuming response deserialization into an appropriate type, return it or part of it
    Ok(HttpResponse::Ok().json(response)) // Change as per your needs
}
