use actix_web::{get, web, Responder};
use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::AppConfig;

#[derive(Clone)]
pub struct JWTKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JWTKeys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Deserialize)]
struct AuthCallbackQuery {
    code: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct WorkOSUser {
    object: String,
    id: String,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    email_verified: bool,
    profile_picture_url: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

// For the request payload
#[derive(Serialize)]
struct WorkOSAuthRequest {
    client_id: String,
    client_secret: String,
    grant_type: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitation_code: Option<String>,
}

// For the response
#[derive(Deserialize)]
struct WorkOSAuthResponse {
    user: WorkOSUser,
    organization_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,
    exp: usize,
}

// /workos/callback?code=01HR5WZ128D1Z7P95BDBT0
#[get("/workos/callback")]
async fn auth_callback(
    app_config: web::Data<AppConfig>,
    info: web::Query<AuthCallbackQuery>,
) -> Result<impl Responder, actix_web::Error> {
    let code = &info.code;
    // Exchange the code for user information using the WorkOS API
    // let auth_response = match exchange_code_for_user(code, app_config.get_ref().clone()).await {
    //     Ok(info) => info,
    //     Err(_) => {
    //         // return actix_web::error::ErrorInternalServerError("Failed to exchange code for user")
    //         // .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    //     }
    // };
    //
    let auth_response = exchange_code_for_user(code, app_config.get_ref().clone())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Sign a JWT with the user info
    // let jwt = match sign_jwt(&auth_response.user, app_config.get_ref().clone()) {
    //     Ok(token) => token,
    //     Err(_) => return actix_web::error::ErrorInternalServerError("Failed to sign JWT"),
    // };
    let jwt = sign_jwt(&auth_response.user, app_config.get_ref().clone())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Redirect to the invisibility deep link with the JWT
    let redirect_url = format!("invisibility://auth_callback?token={}", jwt);
    // let redirect_url = "https://google.com";
    info!("Redirecting to: {}", redirect_url);
    Ok(web::Redirect::to(redirect_url))
}

async fn exchange_code_for_user(
    code: &str,
    app_config: AppConfig,
) -> Result<WorkOSAuthResponse, Box<dyn std::error::Error>> {
    // Use a more generic error type to allow for different kinds of errors
    let client = Client::new();
    let response = client
        .post("https://api.workos.com/user_management/authenticate")
        .header(
            "Authorization",
            format!("Bearer {}", app_config.workos_api_key),
        )
        .json(&WorkOSAuthRequest {
            client_id: app_config.workos_client_id.clone(),
            client_secret: app_config.workos_api_key.clone(),
            grant_type: "authorization_code".to_owned(),
            code: code.to_owned(),
            ip_address: None,
            user_agent: None,
            invitation_code: None,
        })
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let auth_response = resp.json::<WorkOSAuthResponse>().await?;
                Ok(auth_response)
            } else {
                // Attempt to read the response body for error details
                let error_body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read response body".to_string());
                error!("Error response from WorkOS: {}", error_body);
                Err("Failed to authenticate user with WorkOS".into())
            }
        }
        Err(e) => {
            error!("HTTP request error: {}", e);
            Err(e.into())
        }
    }
}

fn sign_jwt(
    user_info: &WorkOSUser,
    app_config: AppConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims {
        sub: user_info.id.clone(),
        exp: 100000, // Make sure to replace this with an actual expiration
    };

    encode(&Header::default(), &claims, &app_config.jwt_keys.encoding)
}
