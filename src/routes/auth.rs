use actix_web::{
    get,
    web::{self, Json},
    Error, Responder,
};
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};
use utoipa::OpenApi;

use crate::models::User;
use crate::types::{
    AuthCallbackQuery, Claims, GetUserResponse, WorkOSAuthRequest, WorkOSAuthResponse, WorkOSUser,
};
use crate::AppState;
use crate::{middleware::auth::AuthenticatedUser, AppConfig};

#[derive(OpenApi)]
#[openapi(
    paths(login, signup, refresh_token, get_user,),
    components(schemas(GetUserResponse, WorkOSAuthRequest, WorkOSAuthResponse, WorkOSUser))
)]
pub struct ApiDoc;

/// A redirect to the WorkOS login page
#[utoipa::path(
    get,
    responses((status = 302, description = "Redirect to WorkOS login page"))
)]
#[get("/login")]
async fn login() -> Result<impl Responder, Error> {
    let url = "https://authkit.i.inc/";
    Ok(web::Redirect::to(url))
}

/// A redirect to the WorkOS login page
#[utoipa::path(
    get,
    responses((status = 302, description = "Redirect to WorkOS signup page"))
)]
#[get("/signup")]
async fn signup() -> Result<impl Responder, Error> {
    let url = "https://authkit.i.inc/sign-up";
    Ok(web::Redirect::to(url))
}

/// The callback URL for the WorkOS authentication flow for the desktop app
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
    Ok(web::Redirect::to(redirect_url))
}

/// The callback URL for the WorkOS authentication flow for the web app
#[get("/workos/callback_nextweb")]
async fn auth_callback_nextweb(
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
    let redirect_url = format!("https://chat.i.inc/auth_callback?token={}", jwt);
    Ok(web::Redirect::to(redirect_url))
}

/// The callback URL for the WorkOS authentication flow for the web app
#[get("/workos/callback_nextweb_dev")]
async fn auth_callback_nextweb_dev(
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
    let redirect_url = format!("http://localhost:3000/auth_callback?token={}", jwt);
    Ok(web::Redirect::to(redirect_url))
}

#[derive(Serialize, Deserialize, Debug)]
struct RefreshTokenResponse {
    token: String,
}

/// Refresh the token for an authenticated user, really just generates a new token
#[utoipa::path(
    get,
    responses((status = 200, description = "Refreshed token for user", body = RefreshTokenResponse, content_type = "application/json"))
)]
#[get("/token/refresh")]
async fn refresh_token(
    authenticated_user: AuthenticatedUser,
    app_config: web::Data<Arc<AppConfig>>,
) -> Result<Json<RefreshTokenResponse>, Error> {
    let user_id = authenticated_user.user_id.as_ref();
    let workos_user = user_id_to_user(user_id, app_config.get_ref().clone())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Sign a JWT with the user info
    let jwt = sign_jwt(&workos_user, app_config.get_ref().clone())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    info!("Refreshed token for user {}", workos_user.email);
    Ok(web::Json(RefreshTokenResponse { token: jwt }))
}

/// Get the user information for the authenticated user
#[utoipa::path(
    get,
    responses((status = 200, description = "User information", body = WorkOSUser, content_type = "application/json"))
)]
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

/// Get all users, but only if the authenticated user is an admin
#[get("/users")]
async fn get_users(
    authenticated_user: AuthenticatedUser,
    app_config: web::Data<Arc<AppConfig>>,
) -> Result<Json<Vec<WorkOSUser>>, Error> {
    if authenticated_user.is_admin() {
        let users = fetch_all_users(app_config.get_ref().clone())
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(web::Json(users))
    } else {
        Err(actix_web::error::ErrorForbidden("You are not an admin"))
    }
}

/// Get all users from workos, async then get, create, or update them into the database
#[get("/users/sync/workos")]
async fn sync_users_workos(
    authenticated_user: AuthenticatedUser,
    app_config: web::Data<Arc<AppConfig>>,
    app_state: web::Data<Arc<AppState>>,
) -> Result<Json<Vec<User>>, Error> {
    if authenticated_user.is_admin() {
        let workos_users = fetch_all_users(app_config.get_ref().clone())
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

        match User::get_or_create_or_update_bulk_workos(&app_state.pool, workos_users).await {
            Ok(users) => Ok(web::Json(users)),
            Err(err) => Err(actix_web::error::ErrorInternalServerError(err.to_string())),
        }
    } else {
        Err(actix_web::error::ErrorForbidden("You are not an admin"))
    }
}

/// Get all users, async PATCH them all to KeywordsAI API
#[get("/users/sync/keywords")]
async fn sync_users_keywords(
    authenticated_user: AuthenticatedUser,
    app_config: web::Data<Arc<AppConfig>>,
    app_state: web::Data<Arc<AppState>>,
) -> Result<Json<Vec<User>>, Error> {
    if authenticated_user.is_admin() {
        match User::get_all(&app_state.pool).await {
            Ok(users) => {
                let mut users_to_process: Vec<_> = users
                    .clone()
                    .into_iter()
                    .filter(|user| !user.linked_to_keywords)
                    .collect();
                let shared_client = Arc::new(Client::new());
                let futures = FuturesUnordered::new();
                info!(
                    "Attempting to sync {} users to KeywordsAI",
                    users_to_process.len()
                );

                let max_concurrent_requests = 50;
                let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));
                for user in users_to_process.iter() {
                    // Wait until we can acquire a permit
                    _ = semaphore.clone().acquire_owned().await.map_err(|e| {
                        error!("Failed to acquire semaphore: {}", e);
                        actix_web::error::ErrorInternalServerError(e.to_string())
                    })?;

                    if !user.linked_to_keywords {
                        let user_id = user.id.clone();
                        let user_email = user.email.clone();
                        let user_name = user.full_name();
                        let client_clone = Arc::clone(&shared_client);
                        let api_key = app_config.keywords_api_key.clone();

                        futures.push(async move {
                            let url =
                                format!("https://api.keywordsai.co/api/user/update/{}", user_id);

                            let response = client_clone
                                .patch(&url)
                                .bearer_auth(&api_key)
                                .json(&json!({
                                    "name": user_name,
                                    "email": user_email,
                                }))
                                .send()
                                .await;

                            match response {
                                Ok(resp) => {
                                    if resp.status().is_success() {
                                        info!("User {} linked to KeywordsAI", user_email);
                                        Ok(user_id)
                                    } else {
                                        let error_body = resp.text().await.unwrap_or_else(|_| {
                                            "Failed to read response body".to_string()
                                        });
                                        warn!("Error response from KeywordsAI: {}", error_body);
                                        Err(())
                                    }
                                }
                                Err(e) => {
                                    error!("HTTP request error: {}", e);
                                    Err(())
                                }
                            }
                        });
                    }
                }

                let results: Vec<Result<String, ()>> = futures.collect().await;

                let mut users_to_update = Vec::new();
                for (user, result) in users_to_process.iter_mut().zip(results) {
                    if let Ok(user_id) = result {
                        user.linked_to_keywords = true;
                        users_to_update.push(user_id);
                    }
                }

                info!("Updating {} users", users_to_update.len());

                sqlx::query!(
                    "UPDATE users SET linked_to_keywords = true WHERE id = ANY($1)",
                    &users_to_update
                )
                .execute(&app_state.pool)
                .await
                .map_err(|e| {
                    error!("Failed to update users: {}", e);
                    actix_web::error::ErrorInternalServerError(e.to_string())
                })?;

                Ok(web::Json(users))
            }
            Err(err) => Err(actix_web::error::ErrorInternalServerError(err.to_string())),
        }
    } else {
        Err(actix_web::error::ErrorForbidden("You are not an admin"))
    }
}

/// Look up a user by ID using the WorkOS API and return the user information
pub async fn user_id_to_user(
    user_id: &str,
    app_config: Arc<AppConfig>,
) -> Result<WorkOSUser, anyhow::Error> {
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
                Err(anyhow::anyhow!("Failed to fetch user from WorkOS"))
            }
        }
        Err(e) => {
            error!("HTTP request error: {}", e);
            Err(e.into())
        }
    }
}

pub async fn fetch_all_users(app_config: Arc<AppConfig>) -> Result<Vec<WorkOSUser>, anyhow::Error> {
    let client = Client::new();
    let mut users = Vec::new();
    let mut after: Option<String> = None;

    loop {
        // Build the request URL, including pagination
        let mut url = "https://api.workos.com/user_management/users".to_string();
        if let Some(ref after_id) = after {
            url = format!("{}?limit=100&after={}", url, after_id);
        }

        let response = client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", app_config.workos_api_key),
            )
            .send()
            .await?;

        if response.status().is_success() {
            let user_list: GetUserResponse = response.json().await?;
            users.extend(user_list.data);

            if let Some(next_after) = user_list.list_metadata.after {
                after = Some(next_after);
            } else {
                break; // No more pages
            }
        } else {
            // Attempt to read the response body for error details
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            error!("Error response from WorkOS: {}", error_body);
            return Err(anyhow::anyhow!("Failed to fetch users from WorkOS"));
        }
    }

    Ok(users)
}

/// Look up a user by email using the WorkOS API and return the user information
pub async fn user_email_to_user(
    user_email: &str,
    app_config: Arc<AppConfig>,
) -> Result<WorkOSUser, anyhow::Error> {
    let client = Client::new();
    let response = client
        .get(format!(
            "https://api.workos.com/user_management/users/?email={}",
            user_email
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
                let user = resp.json::<GetUserResponse>().await?;
                Ok(user.data[0].clone())
            } else {
                // Attempt to read the response body for error details
                let error_body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read response body".to_string());
                error!("Error response from WorkOS: {}", error_body);
                Err(anyhow::anyhow!("Failed to fetch user from WorkOS"))
            }
        }
        Err(e) => {
            error!("HTTP request error: {}", e);
            Err(e.into())
        }
    }
}

/// Exchange the WorkOS provided code for user information using the WorkOS API
async fn exchange_code_for_user(
    code: &str,
    app_config: Arc<AppConfig>,
) -> Result<WorkOSAuthResponse, anyhow::Error> {
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
                Err(anyhow::anyhow!("Failed to authenticate with WorkOS"))
            }
        }
        Err(e) => {
            error!("HTTP request error: {}", e);
            Err(e.into())
        }
    }
}

/// Sign a JWT with the user info. By default, the token expires after 5 weeks. Returns the JWT.
fn sign_jwt(
    user_info: &WorkOSUser,
    app_config: Arc<AppConfig>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_info.id.clone(),
        exp: now + 3600 * 24 * 7 * 5, // Token expires after 5 weeks
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(app_config.jwt_secret.as_ref()),
    )
}
