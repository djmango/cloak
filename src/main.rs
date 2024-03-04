use actix_web::middleware::Logger;
use actix_web::{get, web};
use anyhow::anyhow;
use async_openai::{config::OpenAIConfig, Client};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_secrets::SecretStore;

mod auth;
mod middleware;
mod oai;

#[get("/")]
async fn hello_world() -> &'static str {
    "Hello World!"
}

#[derive(Clone)]
pub struct AppConfig {
    pub openai_api_key: String,
    pub jwt_keys: auth::JWTKeys,
}

impl AppConfig {
    // Asynchronous factory function for creating AppConfig
    pub fn new(secret_store: &SecretStore) -> Result<Self, anyhow::Error> {
        let openai_api_key = secret_store
            .get("OPENAI_API_KEY")
            .ok_or_else(|| anyhow!("OPENAI_API_KEY not found"))?;

        let jwt_secret = secret_store
            .get("JWT_SECRET")
            .ok_or_else(|| anyhow!("JWT_SECRET not found"))?;

        let jwt_keys = auth::JWTKeys::new(jwt_secret.as_bytes());

        Ok(AppConfig {
            openai_api_key,
            jwt_keys,
        })
    }
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

    let app_config = AppConfig::new(&secret_store).unwrap();
    let state = AppState {
        oai_client: Client::with_config(
            OpenAIConfig::new().with_api_key(app_config.openai_api_key.clone()),
        ),
    };

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("")
                .service(hello_world)
                .service(web::scope("/oai").service(oai::chat))
                .wrap(Logger::new(
                    "%t %{r}a \"%r\" %s %b \"%{User-Agent}i\" %U %T",
                ))
                // This pattern breaks down as follows:
                // • %t: Timestamp
                // • %r: First line of the request (method and path)
                // • %s: Response status code
                // • %b: Size of response in bytes, excluding HTTP headers
                // • %{Referer}i: The value of the Referer header
                // • %User-Agent}i: The value of the User-Agent header
                // • %T: Time taken to serve the request, in seconds
                .app_data(web::Data::new(state.clone()))
                .app_data(web::Data::new(app_config.clone())),
        );
    };

    Ok(config.into())
}
