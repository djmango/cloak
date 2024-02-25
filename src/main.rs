use actix_web::{dev::PeerAddr, error, get, web, Error, HttpRequest, HttpResponse, Responder};
// use awc::Client;
use shuttle_actix_web::ShuttleActixWeb;
use tokio::time::sleep;

use bytes::Bytes;
use futures::{future, stream, StreamExt};
use rand::{rngs::ThreadRng, thread_rng, Rng};
use std::convert::Infallible;
use std::time::Duration;

#[get("/")]
async fn hello_world() -> &'static str {
    "Hello World!"
}

#[get("/random")]
async fn random_text_streamer() -> HttpResponse {
    // Define the stream using an async generator block
    let generated_stream = stream::unfold(
        (0usize, thread_rng()),
        |(state, mut rng): (usize, ThreadRng)| async move {
            if state >= 50 {
                // Stop after 500 characters
                return None;
            }

            let random_chars: String = (0..10)
                .map(|_| rng.gen_range(b'a'..=b'z') as char)
                .collect();

            // Properly uncomment and use sleep to introduce a delay between chunks
            log::info!("Sleeping for 1 second");
            sleep(Duration::from_secs(1)).await;
            log::info!(
                "Woke up after 1 second, generated random text: {}",
                random_chars
            );

            Some((
                Ok::<_, Infallible>(Bytes::from(random_chars)),
                (state + 1, rng),
            ))
        },
    );

    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/plain; charset=utf-8")) // Optional: Explicitly set Content-Type
        .streaming(generated_stream)
}

#[get("/streamr")]
async fn streamr() -> HttpResponse {
    let body = stream::once(future::ok::<_, Error>(web::Bytes::from_static(b"test")));

    HttpResponse::Ok()
        .content_type("application/json")
        .streaming(body)
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

        // Return the chunk and the next state
        Some((Ok::<Bytes, Error>(Bytes::from(chunk)), state + 1))
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .streaming(body)
}

#[shuttle_runtime::main]
async fn main() -> ShuttleActixWeb<impl FnOnce(&mut web::ServiceConfig) + Send + Clone + 'static> {
    std::env::set_var("RUST_LOG", "debug");

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(hello_world);
        cfg.service(random_text_streamer);
        cfg.service(streamr);
        cfg.service(streamrr);
    };

    Ok(config.into())
}
