use actix_web::middleware::Logger;
use actix_web::{get, web};
use async_openai::{config::OpenAIConfig, Client};
use config::AppConfig;
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use std::collections::HashMap;
use std::sync::Arc;

mod config;
mod middleware;
mod routes;

#[get("/")]
async fn hello_world() -> &'static str {
    "Hello World!"
}

#[derive(Clone)]
struct AppState {
    persist: PersistInstance,
    oai_client: Client<OpenAIConfig>,
    openrouter_client: Client<OpenAIConfig>,
    stripe_client: stripe::Client,
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_runtime::Secrets] secret_store: SecretStore,
    #[shuttle_persist::Persist] persist: PersistInstance,
) -> ShuttleActixWeb<impl FnOnce(&mut web::ServiceConfig) + Send + Clone + 'static> {
    let app_config = Arc::new(AppConfig::new(&secret_store).unwrap());
    let app_state = Arc::new(AppState {
        persist,
        oai_client: Client::with_config(
            OpenAIConfig::new().with_api_key(app_config.openai_api_key.clone()),
        ),
        openrouter_client: Client::with_config(
            OpenAIConfig::new()
                .with_api_key(app_config.openrouter_api_key.clone())
                // .with_api_base("https://openrouter.ai/api/v1"),
                .with_api_base("https://gateway.hconeai.com/api/v1")
                .with_additional_headers(HashMap::from_iter(
                    vec![
                        (
                            "Helicone-Auth".to_string(),
                            format!("Bearer {}", app_config.helicone_api_key),
                        ),
                        (
                            "Helicone-Target-Url".to_string(),
                            "https://openrouter.ai".to_string(),
                        ),
                        (
                            "Helicone-Target-Provider".to_string(),
                            "OpenRouter".to_string(),
                        ),
                    ]
                    .into_iter(),
                )),
        ),
        stripe_client: stripe::Client::new(app_config.stripe_secret_key.clone()),
    });

    let _guard = sentry::init((
        app_config.sentry_dsn.clone(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            traces_sample_rate: 0.2,
            ..Default::default()
        },
    ));

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("")
                .service(hello_world)
                .service(
                    web::scope("/oai")
                        .service(routes::oai::chat)
                        .app_data(web::JsonConfig::default().limit(1024 * 1024 * 50)), // 50 MB
                )
                .service(
                    web::scope("/auth")
                        .service(routes::auth::auth_callback)
                        .service(routes::auth::get_user)
                        .service(routes::auth::login)
                        .service(routes::auth::refresh_token),
                )
                .service(
                    web::scope("/pay")
                        .service(routes::pay::checkout)
                        .service(routes::pay::invite)
                        .service(routes::pay::manage)
                        .service(routes::pay::paid)
                        .service(routes::pay::payment_success),
                )
                .wrap(middleware::auth::AuthenticationMiddleware {
                    app_config: app_config.clone(),
                })
                .wrap(middleware::logging::LoggingMiddleware)
                .wrap(Logger::new("%{r}a \"%r\" %s %b \"%{User-Agent}i\" %U %T"))
                .wrap(sentry_actix::Sentry::new())
                .app_data(web::Data::new(app_state.clone()))
                .app_data(web::Data::new(app_config.clone())),
        );
    };

    Ok(config.into())
}
