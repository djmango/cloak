use actix_web::middleware::Logger;
use actix_web::{get, web};
use anyhow::anyhow;
use async_openai::{config::OpenAIConfig, Client};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use std::sync::Arc;

mod middleware;
mod routes;

#[get("/")]
async fn hello_world() -> &'static str {
    "Hello World!"
}

#[derive(Clone)]
pub struct AppConfig {
    pub openai_api_key: String,
    pub openrouter_api_key: String,
    pub workos_api_key: String,
    pub workos_client_id: String,
    pub jwt_secret: String,
    pub aws_region: String,
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub stripe_secret_key: String,
    pub sentry_dsn: String,
}

impl AppConfig {
    // Asynchronous factory function for creating AppConfig
    pub fn new(secret_store: &SecretStore) -> Result<Self, anyhow::Error> {
        let openai_api_key = secret_store
            .get("OPENAI_API_KEY")
            .ok_or_else(|| anyhow!("OPENAI_API_KEY not found"))?;

        let openrouter_api_key = secret_store
            .get("OPENROUTER_API_KEY")
            .ok_or_else(|| anyhow!("OPENROUTER_API_KEY not found"))?;

        let workos_api_key = secret_store
            .get("WORKOS_API_KEY")
            .ok_or_else(|| anyhow!("WORKOS_API_KEY not found"))?;

        let workos_client_id = secret_store
            .get("WORKOS_CLIENT_ID")
            .ok_or_else(|| anyhow!("WORKOS_CLIENT_ID not found"))?;

        let jwt_secret = secret_store
            .get("JWT_SECRET")
            .ok_or_else(|| anyhow!("JWT_SECRET not found"))?;

        let aws_region = secret_store
            .get("AWS_REGION")
            .ok_or_else(|| anyhow!("AWS_REGION not found"))?;

        let aws_access_key_id = secret_store
            .get("AWS_ACCESS_KEY_ID")
            .ok_or_else(|| anyhow!("AWS_ACCESS_KEY_ID not found"))?;

        let aws_secret_access_key = secret_store
            .get("AWS_SECRET_ACCESS_KEY")
            .ok_or_else(|| anyhow!("AWS_SECRET_ACCESS_KEY not found"))?;

        let stripe_secret_key = secret_store
            .get("STRIPE_SECRET_KEY")
            .ok_or_else(|| anyhow!("STRIPE_SECRET_KEY not found"))?;

        let sentry_dsn = secret_store
            .get("SENTRY_DSN")
            .ok_or_else(|| anyhow!("SENTRY_DSN not found"))?;

        Ok(AppConfig {
            openai_api_key,
            openrouter_api_key,
            workos_api_key,
            workos_client_id,
            jwt_secret,
            aws_region,
            aws_access_key_id,
            aws_secret_access_key,
            stripe_secret_key,
            sentry_dsn,
        })
    }
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
                .with_api_base("https://openrouter.ai/api/v1"),
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
