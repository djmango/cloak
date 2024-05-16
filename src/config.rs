use anyhow::anyhow;
use shuttle_runtime::SecretStore;

#[derive(Clone)]
pub struct AppConfig {
    pub keywords_api_key: String,
    pub workos_api_key: String,
    pub workos_client_id: String,
    pub jwt_secret: String,
    pub aws_region: String,
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub stripe_secret_key: String,
    pub loops_api_key: String,
}

impl AppConfig {
    // Asynchronous factory function for creating AppConfig
    pub fn new(secret_store: &SecretStore) -> Result<Self, anyhow::Error> {
        let keywords_api_key = secret_store
            .get("KEYWORDS_API_KEY")
            .ok_or_else(|| anyhow!("KEYWORDS_API_KEY not found"))?;

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

        let loops_api_key = secret_store
            .get("LOOPS_API_KEY")
            .ok_or_else(|| anyhow!("LOOPS_API_KEY not found"))?;

        Ok(AppConfig {
            keywords_api_key,
            workos_api_key,
            workos_client_id,
            jwt_secret,
            aws_region,
            aws_access_key_id,
            aws_secret_access_key,
            stripe_secret_key,
            loops_api_key,
        })
    }
}
