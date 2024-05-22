use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::web;
use async_openai::{config::OpenAIConfig, Client};
use config::AppConfig;
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use sqlx::postgres::PgPool;
use std::sync::Arc;

mod config;
mod middleware;
mod models;
mod routes;

#[derive(Clone)]
struct AppState {
    persist: PersistInstance,
    pool: PgPool,
    keywords_client: Client<OpenAIConfig>,
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
        pool: PgPool::connect(&app_config.db_connection_uri)
            .await
            .unwrap(),
        keywords_client: Client::with_config(
            OpenAIConfig::new()
                .with_api_key(app_config.keywords_api_key.clone())
                .with_api_base("https://api.keywordsai.co/api"),
        ),
        stripe_client: stripe::Client::new(app_config.stripe_secret_key.clone()),
    });

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("")
                .service(routes::hello::hello_world)
                .service(
                    web::scope("/auth")
                        .service(routes::auth::auth_callback)
                        .service(routes::auth::get_user)
                        .service(routes::auth::get_users)
                        .service(routes::auth::sync_users_workos)
                        .service(routes::auth::sync_users_keywords)
                        .service(routes::auth::login)
                        .service(routes::auth::refresh_token)
                        .service(routes::auth::signup),
                )
                .service(
                    web::scope("/oai")
                        .service(routes::oai::chat)
                        .app_data(web::JsonConfig::default().limit(1024 * 1024 * 50)), // 50 MB
                )
                .service(
                    web::scope("/pay")
                        .service(routes::pay::checkout)
                        .service(routes::pay::get_invite)
                        .service(routes::pay::list_invites)
                        .service(routes::pay::manage)
                        .service(routes::pay::paid)
                        .service(routes::pay::payment_success),
                )
                .service(web::scope("/sync").service(routes::sync::chats_and_messages))
                .service(web::scope("/webhook").service(routes::webhook::user_created))
                .wrap(middleware::auth::AuthenticationMiddleware {
                    app_config: app_config.clone(),
                })
                .wrap(middleware::logging::LoggingMiddleware)
                .wrap(Logger::new("%{r}a \"%r\" %s %b \"%{User-Agent}i\" %U %T"))
                .wrap(Cors::permissive())
                .app_data(web::Data::new(app_state.clone()))
                .app_data(web::Data::new(app_config.clone())),
        );
    };

    Ok(config.into())
}
