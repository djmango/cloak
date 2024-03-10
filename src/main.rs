use actix_web::middleware::Logger;
use actix_web::{get, web};
use anyhow::anyhow;
use async_openai::{config::OpenAIConfig, Client};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_persist::PersistInstance;
use shuttle_secrets::SecretStore;
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
        })
    }
}

// async fn get_bedrock_client(app_config: &AppConfig) -> aws_sdk_bedrockruntime::Client {
//     // Set env
//     std::env::set_var("AWS_ACCESS_KEY_ID", app_config.aws_access_key_id.clone());
//     std::env::set_var(
//         "AWS_SECRET_ACCESS_KEY",
//         app_config.aws_secret_access_key.clone(),
//     );
//     std::env::set_var("AWS_REGION", app_config.aws_region.clone());

//     let aws_sdk_config = aws_config::load_from_env().await;
//     aws_sdk_bedrockruntime::Client::new(&aws_sdk_config)
// }

#[derive(Clone)]
struct AppState {
    persist: PersistInstance,
    oai_client: Client<OpenAIConfig>,
    openrouter_client: Client<OpenAIConfig>,
    // bedrock_client: aws_sdk_bedrockruntime::Client,
    stripe_client: stripe::Client,
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
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
        // bedrock_client: get_bedrock_client(&app_config).await,
        stripe_client: stripe::Client::new(app_config.stripe_secret_key.clone()),
    });

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("")
                .service(hello_world)
                .service(web::scope("/oai").service(routes::oai::chat))
                .service(
                    web::scope("/auth")
                        .service(routes::auth::login)
                        .service(routes::auth::auth_callback)
                        .service(routes::auth::get_user),
                )
                .service(
                    web::scope("/pay")
                        .service(routes::pay::invite)
                        .service(routes::pay::checkout)
                        .service(routes::pay::paid)
                        .service(routes::pay::payment_success)
                        .service(routes::pay::manage),
                )
                .wrap(middleware::auth::Authentication {
                    app_config: app_config.clone(),
                })
                .wrap(Logger::new(
                    "%t %{r}a \"%r\" %s %b \"%{User-Agent}i\" %U %T",
                ))
                // • %t: Timestamp
                // • %r: First line of the request (method and path)
                // • %s: Response status code
                // • %b: Size of response in bytes, excluding HTTP headers
                // • %{Referer}i: The value of the Referer header
                // • %User-Agent}i: The value of the User-Agent header
                // • %T: Time taken to serve the request, in seconds
                .app_data(web::Data::new(app_state.clone()))
                .app_data(web::Data::new(app_config.clone())),
        );
    };

    Ok(config.into())
}
