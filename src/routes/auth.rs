use std::sync::Arc;

use actix_web::{
    get,
    web::{self, Json},
    Error, Responder,
};
use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{middleware::auth::AuthenticatedUser, AppConfig};

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
#[allow(dead_code)] // We never really use organization_id but whatever
struct WorkOSAuthResponse {
    user: WorkOSUser,
    organization_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

#[get("/workos/callback")]
async fn auth_callback(
    app_config: web::Data<Arc<AppConfig>>,
    info: web::Query<AuthCallbackQuery>,
) -> Result<impl Responder, actix_web::Error> {
    let code = &info.code;
    // Exchange the code for user information using the WorkOS API
    let auth_response = exchange_code_for_user(code, app_config.get_ref().clone())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Sign a JWT with the user info
    let jwt = sign_jwt(&auth_response.user, app_config.get_ref().clone())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Redirect to the invisibility deep link with the JWT
    let redirect_url = format!("invisibility://auth_callback?token={}", jwt);
    info!("Redirecting to: {}", redirect_url);
    Ok(web::Redirect::to(redirect_url))
}

#[get("/user")]
async fn get_user(
    authenticated_user: AuthenticatedUser,
    app_config: web::Data<Arc<AppConfig>>,
) -> Result<Json<WorkOSUser>, Error> {
    let user_id = authenticated_user.user_id.as_ref();
    let workos_user = user_id_to_user(user_id, app_config.get_ref().clone())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()));

    // Since there's no conditional checking of `AuthenticatedUser`, you directly work with it
    Ok(web::Json(workos_user?))
}

async fn user_id_to_user(
    user_id: &str,
    app_config: Arc<AppConfig>,
) -> Result<WorkOSUser, Box<dyn std::error::Error>> {
    let client = Client::new();
    let response = client
        .get(format!(
            "https://api.workos.com/user_management/users/{}",
            user_id
        ))
        .header(
            "Authorization",
            format!("Bearer {}", app_config.workos_api_key),
        )
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let user = resp.json::<WorkOSUser>().await?;
                Ok(user)
            } else {
                // Attempt to read the response body for error details
                let error_body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read response body".to_string());
                error!("Error response from WorkOS: {}", error_body);
                Err("Failed to fetch user from WorkOS".into())
            }
        }
        Err(e) => {
            error!("HTTP request error: {}", e);
            Err(e.into())
        }
    }
}

async fn exchange_code_for_user(
    code: &str,
    app_config: Arc<AppConfig>,
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
    app_config: Arc<AppConfig>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_info.id.clone(),
        exp: now + 3600 * 24 * 7, // Token expires after 1 week
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(app_config.jwt_secret.as_ref()),
    )
}
