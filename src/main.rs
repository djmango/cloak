use crate::models::memory::Memory;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::web;
use async_openai::{config::OpenAIConfig, Client};
use chrono::Utc;
use config::AppConfig;
use futures::future::join_all;
use futures::stream::{self, StreamExt};
use models::User;
use moka::future::Cache;
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use rand::seq::SliceRandom;
use sqlx::postgres::PgPool;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, debug};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
use uuid::Uuid;

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

    /*
    let scheduler = JobScheduler::new().await.unwrap();
    let app_state_clone: Arc<AppState> = app_state.clone();
    let yesterday: chrono::prelude::DateTime<Utc> = Utc::now() - chrono::Duration::days(1);
    */

    // Production code (commented out)
    // let job = Job::new_async("0 0 0 * * *", move |_uuid, _l| {
    //     let app_state: Arc<AppState> = app_state_clone.clone();
    //     Box::pin(async move {
    //         let timeout = Duration::from_secs(3 * 60 * 60); // 3 hours
    //         match tokio::time::timeout(timeout, generate_all_users_memories(app_state, yesterday)).await {
    //             Ok(_) => info!("Job completed successfully within the time limit"),
    //             Err(_) => error!("Job timed out after 3 hours"),
    //         }
    //     })
    // })
    // .unwrap();
    // scheduler.add(job).await.unwrap();
    // scheduler.start().await.unwrap();

    // Test version (executes immediately)
    /*
    let app_state_clone: Arc<AppState> = app_state.clone();
    let timeout = Duration::from_secs(3 * 60 * 60); // 3 hours
    let begin_time = Utc::now() - chrono::Duration::days(365 * 10); // 10 years ago
    tokio::spawn(async move {
        match tokio::time::timeout(timeout, generate_all_users_memories(app_state_clone, begin_time)).await {
            Ok(_) => info!("Test job completed successfully within the time limit"),
            Err(_) => error!("Test job timed out after 3 hours"),
        }
    });
*/
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
                    web::scope("/memories")
                        .service(routes::memory::generate_memories_from_chat_history_endpoint)
                        .service(routes::memory::create_memory)
                        .service(routes::memory::get_memories)
                        .service(routes::memory::update_memory)
                        .service(routes::memory::delete_memory),
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

fn select_random_fraction(users: &[User], fraction: f64) -> Vec<User> {
    let mut rng = rand::thread_rng();
    let sample_size = (users.len() as f64 * fraction).ceil() as usize;
    users.choose_multiple(&mut rng, sample_size).cloned().collect()
}

async fn generate_all_users_memories(app_state: Arc<AppState>, begin_time: chrono::DateTime<Utc>) {
    let all_users = match User::get_all(&app_state.pool).await {
        Ok(users) => users,
        Err(e) => {
            debug!("Failed to get all users: {:?}", e);
            return;
        }
    };
    info!("Total users: {}", all_users.len());

    let selected_users = select_random_fraction(&all_users, 0.05);
    let selected_users_count = selected_users.len();
    info!("Selected users: {}", selected_users_count);

    let batch_size = 100;
    let semaphore = Arc::new(Semaphore::new(batch_size));

    let successful_futures = stream::iter(selected_users)
        .chunks(batch_size)
        .flat_map(|chunk| {
            let app_state = app_state.clone();
            let semaphore = semaphore.clone();
            stream::iter(chunk).map(move |user| {
                let app_state = app_state.clone();
                let user_id = user.id;
                let semaphore = semaphore.clone();
                async move {
                    let response = routes::memory::generate_memories_from_chat_history(
                        &web::Data::new(app_state),
                        Some(semaphore),
                        &user_id,
                        None,
                        None,
                        Some((begin_time, Utc::now())),
                    )
                    .await;
                    match response {
                        Ok(memories) => {
                            info!("Memories generated successfully for user: {}. Count: {}", user_id, memories.len());
                            Some(user_id)
                        },
                        Err(e) => {
                            error!("Error generating memories for user {}: {:?}", user_id, e);
                            None
                        },
                    }
                }
            })
        })
        .buffer_unordered(batch_size)
        .filter_map(|result| async move { result })
        .collect::<Vec<_>>()
        .await;

    let processed_users_count = successful_futures.len();

    info!("Memory generation complete. Breakdown:");
    info!("Selected users: {}", selected_users_count);
    info!("Processed users: {}", processed_users_count);
    info!("Not processed users: {}", selected_users_count - processed_users_count);
}