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

    let request = (|| -> Result<_, anyhow::Error> {
        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model("gpt-3.5-turbo")
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content("You are a helpful assistant.")
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content("Who won the world series in 2020?")
                    .build()?
                    .into(),
                ChatCompletionRequestAssistantMessageArgs::default()
                    .content("The Los Angeles Dodgers won the World Series in 2020.")
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content("Where was it played? And the next one")
                    .build()?
                    .into(),
            ])
            .build()?;
        Ok(request)
    })();

    match request {
        Ok(response) => {
            // Call API
            let response = state.oai_client.chat().create(response).await.unwrap();

            info!("{}", response.choices.first().unwrap().index);
            let content = response
                .choices
                .first()
                .unwrap()
                .message
                .content
                .as_ref()
                .unwrap()
                .clone();

            Ok(HttpResponse::Ok().content_type("text/plain").body(content))
        }
        Err(err) => {
            // Convert anyhow::Error to actix_web::Error
            Err(actix_web::error::ErrorInternalServerError(err.to_string()))
        }
    }
}
