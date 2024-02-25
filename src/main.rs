use actix_web::{get, web, Error, HttpResponse, Responder};
use anyhow::anyhow;
use bytes::Bytes;
use futures::stream;
use rand::{thread_rng, Rng};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_secrets::SecretStore;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

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

#[get("/streamrr")]
async fn streamrr() -> impl Responder {
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

#[shuttle_runtime::main]
async fn main(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> ShuttleActixWeb<impl FnOnce(&mut web::ServiceConfig) + Send + Clone + 'static> {
    color_eyre::install().unwrap();

    // Verify that the required secrets at least exist
    // Does not verify that they are valid
    let secret_keys = ["OPENAI_API_KEY", "TOGETHERAI_API_KEY"];
    for key in secret_keys.iter() {
        match secret_store.get(key) {
            Some(_) => {
                info!("{} Set", key);
            }
            None => {
                return Err(anyhow!("{} was not found", key).into());
            }
        }
    }

    // Configure the Actix Web service
    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(hello_world);
        cfg.service(streamrr);
    };

    Ok(config.into())
}
