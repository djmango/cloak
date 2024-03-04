use actix_web::{get, web, Error, HttpResponse, Responder};
use anyhow::anyhow;
use async_openai::{config::OpenAIConfig, Client};
use bytes::Bytes;
use futures::stream;
use rand::{thread_rng, Rng};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_secrets::SecretStore;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

mod oai;

#[get("/")]
async fn hello_world() -> &'static str {
    "Hello World!"
}

async fn generate_random_chars() -> String {
    let mut rng = thread_rng();
    (0..10)
        .map(|_| (rng.gen_range(b'a'..=b'z') as char))
        .collect()
}

#[get("/stream_rand")]
async fn stream_rand() -> impl Responder {
    // Simulate streaming of data by sending a chunk every second
    let body = stream::unfold(0, |state| async move {
        // Stop after 5 chunks
        if state >= 5 {
            return None;
        }

        // Simulate some processing delay
        sleep(Duration::from_secs(1)).await;

        // Generate chunk
        // let chunk = generate_random_chars().await;
        // let chunk = format!("{}", generate_random_chars().await);
        let chunk = format!("Chunk {}: test\n", generate_random_chars().await);
        // ITS JUST THE NEWLINE

        // Return the chunk and the next state
        Some((Ok::<Bytes, Error>(Bytes::from(chunk)), state + 1))
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .streaming(body)
}

struct AppConfig {
    pub openai_api_key: String,
    pub togetherai_api_key: String,
}

#[derive(Clone)]
struct AppState {
    oai_client: Client<OpenAIConfig>,
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> ShuttleActixWeb<impl FnOnce(&mut web::ServiceConfig) + Send + Clone + 'static> {
    color_eyre::install().unwrap();

    let mut app_config = AppConfig {
        openai_api_key: String::new(),
        togetherai_api_key: String::new(),
    };

    // Verify that the required secrets at least exist
    // Does not verify that they are valid
    let secret_keys = ["OPENAI_API_KEY", "TOGETHERAI_API_KEY"];
    for key in secret_keys.iter() {
        match secret_store.get(key) {
            Some(_) => match *key {
                "OPENAI_API_KEY" => {
                    app_config.openai_api_key = secret_store.get(key).unwrap();
                    info!("{} Set", key);
                }
                "TOGETHERAI_API_KEY" => {
                    app_config.togetherai_api_key = secret_store.get(key).unwrap();
                    info!("{} Set", key);
                }
                _ => {
                    info!("{} Set", key);
                    warn!("{} is not a known secret", key);
                }
            },
            None => {
                return Err(anyhow!("{} was not found", key).into());
            }
        }
    }

    let config = OpenAIConfig::new().with_api_key(app_config.openai_api_key);

    let state = AppState {
        oai_client: Client::with_config(config),
    };

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("/oai")
                .service(hello_world)
                .service(oai::ai)
                .service(oai::chat),
        )
        .service(web::scope("").service(hello_world).service(stream_rand))
        .app_data(web::Data::new(state.clone()));
    };

    Ok(config.into())
}
