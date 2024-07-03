use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::web;
use async_openai::{config::OpenAIConfig, Client};
use config::AppConfig;
use futures::future::join_all;
use models::User;
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
use moka::future::Cache;
use crate::models::memory::Memory;
use uuid::Uuid;
use std::collections::HashMap;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};
use chrono::Utc;

mod config;
mod middleware;
mod models;
mod prompts;
mod routes;
mod types;

#[derive(Clone)]
struct AppState {
    persist: PersistInstance,
    pool: PgPool,
    keywords_client: Client<OpenAIConfig>,
    stripe_client: stripe::Client,
    memory_cache: Cache<String, HashMap<Uuid, Memory>>,
}

#[derive(OpenApi)]
#[openapi(
        nest(
            (path = "/", api = routes::hello::ApiDoc),
            (path = "/auth", api = routes::auth::ApiDoc),
            (path = "/chats", api = routes::chat::ApiDoc),
            (path = "/pay", api = routes::pay::ApiDoc),
            (path = "/oai", api = routes::oai::ApiDoc),
            (path = "/sync", api = routes::sync::ApiDoc),
        ),
        tags(
            (name = "cloak", description = "Invisibiliy cloak API, powering i.inc and related services.")
        )
    )]
struct ApiDoc;

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
        memory_cache: Cache::builder()
            .max_capacity(1024 * 1024 * 1024) // 1GB limit (in bytes)
            .weigher(|_key, value: &HashMap<Uuid, Memory>| -> u32 {
                let estimated_memory_size = 1000; // Assume each Memory object is roughly 1000 bytes
                (value.len() * estimated_memory_size) as u32
            })
            .build(),
    });

    let scheduler = JobScheduler::new().await.unwrap();
    let app_state_clone: Arc<AppState> = app_state.clone();
    let yesterday = Utc::now() - chrono::Duration::days(1);

    let job = Job::new_async("0 0 0 * * *", move |_uuid, _l| {   
        let app_state: Arc<AppState> = app_state_clone.clone();

        Box::pin(async move {
            let all_users = User::get_all(
                &app_state.pool,
            ).await.unwrap();
        
            info!("All users: {:?}", all_users.len());
        
            let mut idx = 0;
            let batch_size = 100;

            while idx < all_users.len() {
                let user_ids = all_users.iter().map(|user| user.id.clone()).skip(idx).take(batch_size).collect::<Vec<String>>();
                
                let futures: Vec<_> = user_ids.iter().enumerate().map(|(i, user_id)| {
                    let app_state = app_state.clone();
                    let user_id = user_id.clone();
                    let i = i.clone();

                    async move {
                        let response = routes::memory::generate_memories_from_chat_history(
                            &web::Data::new(app_state), 
                            &user_id, 
                            &Uuid::parse_str("b66ebb74-09c2-4c67-bf99-52c05e7dbe44").unwrap(), 
                            None, 
                            None,
                            Some(yesterday)
                        ).await;

                        match response {
                            Ok(_res) => {
                                info!("Memories generated successfully: {:?}", i);
                            }
                            Err(e) => {
                                error!("Error generating memories: {:?}", e);
                            }
                        }
                    }
                }).collect();

                join_all(futures).await;
                idx += batch_size;
            }
        })
    })
    .unwrap();

    scheduler.add(job).await.unwrap();
    scheduler.start().await.unwrap();

    let openapi = ApiDoc::openapi();

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("")
                .service(routes::hello::hello_world)
                .service(
                    web::scope("/auth")
                        .service(routes::auth::auth_callback)
                        .service(routes::auth::auth_callback_nextweb)
                        .service(routes::auth::auth_callback_nextweb_dev)
                        .service(routes::auth::get_user)
                        .service(routes::auth::get_users)
                        .service(routes::auth::sync_users_workos)
                        .service(routes::auth::sync_users_keywords)
                        .service(routes::auth::login)
                        .service(routes::auth::refresh_token)
                        .service(routes::auth::signup),
                )
                .service(
                    web::scope("/chats")
                        .service(routes::chat::delete_chat)
                        .service(routes::chat::update_chat)
                        .service(routes::chat::autorename_chat),
                )
                .service(
                    web::scope("/messages")
                        .service(routes::messages::upvote_message)
                        .service(routes::messages::downvote_message),
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
                .service(
                    web::scope("/memory")
                        .service(routes::memory::generate_memories_from_chat_history_endpoint)
                        .service(routes::memory::add_memory_prompt)
                        .service(routes::memory::create_memory)
                        .service(routes::memory::get_all_memories)
                        .service(routes::memory::update_memory)
                        .service(routes::memory::delete_memory)
                        .service(routes::memory::delete_all_memories)
                )
                .service(web::scope("/sync").service(routes::sync::sync_all))
                .service(web::scope("/webhook").service(routes::webhook::user_created))
                .service(Scalar::with_url("/scalar", openapi))
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
