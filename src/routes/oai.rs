use actix_web::{post, web, Error, HttpResponse, Responder};
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::{CreateChatCompletionRequest, CreateChatCompletionStreamResponse};
use async_openai::Client;
// use aws_sdk_bedrockruntime::error::SdkError;
// use aws_sdk_bedrockruntime::operation::invoke_model_with_response_stream::{
//     InvokeModelWithResponseStreamError, InvokeModelWithResponseStreamOutput,
// };
// use aws_sdk_bedrockruntime::primitives::Blob;
// use aws_smithy_runtime_api::http::response::Response;
use bytes::Bytes;
use futures::stream::StreamExt;
use serde_json::to_string;
use tracing::info;

use crate::middleware::auth::AuthenticatedUser;
use crate::AppState;

#[post("/v1/chat/completions")]
async fn chat(
    app_state: web::Data<AppState>,
    authenticated_user: AuthenticatedUser,
    req_body: web::Json<CreateChatCompletionRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let user_id = &authenticated_user.user_id;

    // info!("AI endpoint hit with model: {}", req_body.model);
    info!(
        "User {} hit the AI endpoint with model: {}",
        user_id, req_body.model
    );

    let mut request_args = req_body.into_inner();

    request_args.stream = Some(true);
    // request_args.model = "openrouter/auto".to_string();
    // request_args.model can either be "invis/claude3_auto" or anything else, which will route to
    // the standard openai model

    // let stream: futures::stream::BoxStream<Result<Bytes, Error>>;

    // match request_args.model.as_str() {
    //     "invis/claude3_auto" => {
    //         let model_id = "anthropic.claude-3-sonnet-20240229-v1:0";
    //         let body = r#"{
    //   "anthropic_version": "bedrock-2023-05-31",
    //   "max_tokens": 1000,
    //   "messages": [
    //     {
    //       "role": "user",
    //       "content": [
    //         {
    //           "type": "text",
    //           "text": "What's in this image?"
    //         }
    //       ]
    //     }
    //   ]
    // }"#;

    //         let body_blob = Blob::new(body);

    //         let response = app_state
    //             .bedrock_client
    //             .invoke_model_with_response_stream()
    //             .body(body_blob)
    //             .model_id(model_id)
    //             .content_type("application/json")
    //             .send()
    //             .await;
    //
    //         response

    //         stream = response
    //             .map(
    //                 |item_result: Result<
    //                     InvokeModelWithResponseStreamOutput,
    //                     SdkError<InvokeModelWithResponseStreamError, Response>,
    //                 >| {
    //                     match item_result {
    //                         Ok(item) => to_string(&item)
    //                             .map_err(actix_web::error::ErrorInternalServerError)
    //                             .map(|json_string| {
    //                                 Bytes::from(format!("data: {}\n\n", json_string))
    //                             }),
    //                         Err(e) => {
    //                             Err(actix_web::error::ErrorInternalServerError(e.to_string()))
    //                         }
    //                     }
    //                 },
    //             )
    //             .boxed();

    // match res {
    //     Ok(res) => {
    //         info!("Response: {:?}", res);
    //         // info!("Response body: {:?}", res.body.into_inner().as_string());
    //         info!(
    //             "Response body: {:?}",
    //             String::from_utf8_lossy(res.body.into_inner().as_ref())
    //         );
    //     }
    //     Err(e) => {
    //         error!("Error: {:?}", e);
    //     }
    // }
    // }
    // _ => {
    //     }
    // }

    // If we want to use claude, use the openrouter client, otherwise use the standard openai client
    let client: Client<OpenAIConfig> = match request_args.model.as_str() {
        "anthropic/claude-3-opus:beta" => app_state.openrouter_client.clone(),
        _ => app_state.oai_client.clone(),
    };

    // let response = app_state
    //     .oai_client
    //     .chat()
    //     .create_stream(request_args)
    //     .await
    //     .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let response = client
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

    // let stream: futures::stream::BoxStream<Result<Bytes, Error>> = response
    //     .map(
    //         |item_result: Result<CreateChatCompletionStreamResponse, OpenAIError>| match item_result
    //         {
    //             Ok(item) => to_string(&item)
    //                 .map_err(actix_web::error::ErrorInternalServerError)
    //                 .map(|json_string| {
    //                     let loggable_value = format!("data: {}\n\n", json_string);
    //                     info!(
    //                         "Transmitting: {}",
    //                         loggable_value.strip_suffix("\n\n").unwrap()
    //                     );

    //                     Bytes::from(loggable_value)
    //                 }),
    //             Err(e) => Err(actix_web::error::ErrorInternalServerError(e.to_string())),
    //         },
    //     )
    //     .boxed();

    Ok(HttpResponse::Ok()
        .content_type("application/stream+json")
        .streaming(stream))
}
