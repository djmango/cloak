use actix_web::{post, web, Error, HttpResponse, Responder};
use async_openai::error::OpenAIError;
use async_openai::types::{CreateChatCompletionRequest, CreateChatCompletionStreamResponse};
use bytes::Bytes;
use futures::stream::StreamExt;
use serde_json::to_string;
use tracing::info;

use crate::AppState;

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
    // let stream: futures::stream::BoxStream<Result<Bytes, Error>> = response
    //     .map(
    //         |item_result: Result<CreateChatCompletionStreamResponse, OpenAIError>| match item_result
    //         {
    //             Ok(item) => to_string(&item)
    //                 .map_err(actix_web::error::ErrorInternalServerError)
    //                 .map(|json_string| Bytes::from(format!("data: {}\n\n", json_string))),
    //             Err(e) => Err(actix_web::error::ErrorInternalServerError(e.to_string())),
    //         },
    //     )
    //     .boxed();

    let stream: futures::stream::BoxStream<Result<Bytes, Error>> = response
        .map(
            |item_result: Result<CreateChatCompletionStreamResponse, OpenAIError>| match item_result
            {
                Ok(item) => to_string(&item)
                    .map_err(actix_web::error::ErrorInternalServerError)
                    .map(|json_string| {
                        let loggable_value = format!("data: {}\n\n", json_string);
                        info!(
                            "Transmitting: {}",
                            loggable_value.strip_suffix("\n\n").unwrap()
                        );

                        Bytes::from(loggable_value)
                    }),
                Err(e) => Err(actix_web::error::ErrorInternalServerError(e.to_string())),
            },
        )
        .boxed();

    Ok(HttpResponse::Ok()
        .content_type("application/stream+json")
        .streaming(stream))
}
