use actix_web::{get, post, web, Error, HttpResponse, Responder};
use async_openai::error::OpenAIError;
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, CreateChatCompletionStreamResponse,
};
use bytes::Bytes;
use futures::stream::StreamExt;
use serde_json::to_string;
use tracing::info; // For serializing your data to a JSON String

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

    let mut request_args = req_body.into_inner();

    // match request_args.stream {
    //     Some(stream) => {
    //         info!("Stream: {}", stream);
    //     }
    //     None => {
    //         info!("No stream");
    //     }
    // }
    request_args.stream = Some(true);

    let response = state
        .oai_client
        .chat()
        .create_stream(request_args)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Construct a streaming HTTP response
    let stream: futures::stream::BoxStream<Result<Bytes, Error>> = response
        .map(
            |item_result: Result<CreateChatCompletionStreamResponse, OpenAIError>| match item_result
            {
                Ok(item) => to_string(&item)
                    .map_err(actix_web::error::ErrorInternalServerError)
                    .map(|json_string| Bytes::from(format!("data: {}\n\n", json_string))),
                Err(e) => Err(actix_web::error::ErrorInternalServerError(e.to_string())),
            },
        )
        .boxed();

    Ok(HttpResponse::Ok()
        .content_type("application/stream+json")
        .streaming(stream))
}
