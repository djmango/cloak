use actix_web::{get, web};
use anyhow::anyhow;
use async_openai::{config::OpenAIConfig, Client};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_secrets::SecretStore;
use tracing::{info, warn};

mod oai;

#[get("/")]
async fn hello_world() -> &'static str {
    "Hello World!"
}

struct AppConfig {
    pub openai_api_key: String,
    pub togetherai_api_key: String,
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

    let mut app_config = AppConfig {
        openai_api_key: String::new(),
        togetherai_api_key: String::new(),
    };

    // Verify that the required secrets at least exist
    // Does not verify that they are valid
    let secret_keys = ["OPENAI_API_KEY", "TOGETHERAI_API_KEY"];
    for key in secret_keys.iter() {
        match secret_store.get(key) {
            Some(_) => match *key {
                "OPENAI_API_KEY" => {
                    app_config.openai_api_key = secret_store.get(key).unwrap();
                    info!("{} Set", key);
                }
                "TOGETHERAI_API_KEY" => {
                    app_config.togetherai_api_key = secret_store.get(key).unwrap();
                    info!("{} Set", key);
                }
                _ => {
                    info!("{} Set", key);
                    warn!("{} is not a known secret", key);
                }
            },
            None => {
                return Err(anyhow!("{} was not found", key).into());
            }
        }
    }

    let config = OpenAIConfig::new().with_api_key(app_config.openai_api_key);

    let state = AppState {
        oai_client: Client::with_config(config),
    };

    let config = move |cfg: &mut web::ServiceConfig| {
        cfg.service(web::scope("/oai").service(hello_world).service(oai::chat))
            .service(web::scope("").service(hello_world))
            .app_data(web::Data::new(state.clone()));
    };

    Ok(config.into())
}
