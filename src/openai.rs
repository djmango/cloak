use actix_web::{get, web, HttpResponse, Responder};
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
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
