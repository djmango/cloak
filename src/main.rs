use actix_web::{dev::PeerAddr, error, get, web, Error, HttpRequest, HttpResponse, Responder};
use awc::Client;
use log::info;
use shuttle_actix_web::ShuttleActixWeb;
use tokio::time::sleep;
use url::Url;

use bytes::Bytes;
use futures::{stream, StreamExt};
use rand::{rngs::ThreadRng, Rng};
use std::convert::Infallible;
use std::time::Duration;

#[get("/")]
async fn hello_world() -> &'static str {
    "Hello World!"
}

#[get("/random")]
async fn random_text_streamer() -> impl Responder {
    // Define the stream using an async generator block
    let generated_stream = stream::unfold(
        (0usize, rand::thread_rng()),
        |(state, mut rng): (usize, ThreadRng)| async move {
            if state >= 50 {
                return None; // Stop after 500 characters
            }

            let random_chars: String = (0..10)
                .map(|_| rng.gen_range(b'a'..=b'z') as char)
                .collect();

            // sleep(Duration::from_secs(1)).await; // Sleep for 1 second

            // Some((Ok(Bytes::from(random_chars)), (state + 1, rng)))
            Some((
                Ok::<_, Infallible>(Bytes::from(random_chars)),
                (state + 1, rng),
            ))
        },
    );

    HttpResponse::Ok().streaming(generated_stream)
}

/// Forwards the incoming HTTP request using `awc`.
#[get("/proxy")]
async fn forward(
    req: HttpRequest,
    payload: web::Payload,
    peer_addr: Option<PeerAddr>,
    url: web::Data<Url>,
    client: web::Data<Client>,
) -> Result<HttpResponse, Error> {
    let mut new_url = (**url).clone();
    new_url.set_path(req.uri().path());
    new_url.set_query(req.uri().query());

    let forwarded_req = client
        .request_from(new_url.as_str(), req.head())
        .no_decompress();

    // TODO: This forwarded implementation is incomplete as it only handles the unofficial
    // X-Forwarded-For header but not the official Forwarded one.
    let forwarded_req = match peer_addr {
        Some(PeerAddr(addr)) => {
            forwarded_req.insert_header(("x-forwarded-for", addr.ip().to_string()))
        }
        None => forwarded_req,
    };

    info!("forwarding to {}", new_url);

    let res = forwarded_req
        .send_stream(payload)
        .await
        .map_err(error::ErrorInternalServerError)?;

    let mut client_resp = HttpResponse::build(res.status());
    // Remove `Connection` as per
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.insert_header((header_name.clone(), header_value.clone()));
    }

    Ok(client_resp.streaming(res))
}

#[get("/test")]
async fn index() -> impl Responder {
    "Hello world2!"
}

#[shuttle_runtime::main]
async fn main() -> ShuttleActixWeb<impl FnOnce(&mut web::ServiceConfig) + Send + Clone + 'static> {
    std::env::set_var("RUST_LOG", "debug");

    info!("forwarding to ");

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(hello_world);
        cfg.service(index);
        cfg.service(forward);
        cfg.service(random_text_streamer);
    };

    Ok(config.into())
}
