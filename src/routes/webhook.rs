use actix_web::HttpResponse;
use actix_web::{post, web};
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error};

use crate::config::AppConfig;
use crate::models::user::User;
use crate::types::LoopsContact;
use crate::types::WorkOSCreateUserWebhookPayload;
use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

#[post("/workos/user_created")]
pub async fn user_created(
    app_state: web::Data<Arc<AppState>>,
    app_config: web::Data<Arc<AppConfig>>,
    req: actix_web::HttpRequest,
    body: web::Bytes,
) -> Result<HttpResponse, actix_web::Error> {
    let workos_signature = req
        .headers()
        .get("workos-signature")
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Missing signature"))?
        .to_str()
        .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid signature format"))?;

    let body_str = std::str::from_utf8(&body)
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid body format"))?;

    // Clone body for JSON deserialization while keeping original for signature verification
    let event: WorkOSCreateUserWebhookPayload = serde_json::from_slice(&body)
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid JSON body"))?;

    // Split the signature header into timestamp and signature parts
    let parts: Vec<&str> = workos_signature.split(',').collect();
    if parts.len() != 2 {
        return Err(actix_web::error::ErrorUnauthorized(
            "Invalid signature format",
        ));
    }

    let timestamp_part = parts[0];
    let signature_part = parts[1];

    let issued_timestamp = match timestamp_part.split('=').collect::<Vec<&str>>().as_slice() {
        [_, timestamp] => *timestamp,
        _ => {
            return Err(actix_web::error::ErrorUnauthorized(
                "Invalid timestamp part",
            ))
        }
    };

    let signature_hash = match signature_part.split('=').collect::<Vec<&str>>().as_slice() {
        [_, signature] => *signature,
        _ => {
            return Err(actix_web::error::ErrorUnauthorized(
                "Invalid signature part",
            ))
        }
    };

    // Validate issued timestamp to avoid replay attacks
    let issued_timestamp = issued_timestamp
        .parse::<u64>()
        .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid timestamp"))?;
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| actix_web::error::ErrorInternalServerError("System time error"))?
        .as_millis() as u64;

    let max_allowed_delay = 300_000; // 5 minutes in milliseconds
    if issued_timestamp + max_allowed_delay < current_time {
        return Err(actix_web::error::ErrorUnauthorized("Timestamp is too old"));
    }

    // Construct the message: issued timestamp + "." + request body
    let message = format!("{}.{}", issued_timestamp, body_str);

    // Compute expected HMAC signature
    let secret = app_config.workos_webhook_signature.as_bytes();

    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|_| actix_web::error::ErrorInternalServerError("HMAC initialization error"))?;
    mac.update(message.as_bytes());

    let expected_signature = hex::encode(mac.finalize().into_bytes());

    // Compare signatures
    if expected_signature != signature_hash {
        return Err(actix_web::error::ErrorUnauthorized("Invalid signature"));
    }

    // Okay webhook is now validated

    let workos_users = vec![event.data.clone()];

    User::get_or_create_or_update_bulk_workos(&app_state.pool, workos_users)
        .await
        .map_err(|err| {
            error!("Error creating user from webhook: {}", err);
            actix_web::error::ErrorInternalServerError("Error creating user from webhook")
        })?;

    let loops_contact = LoopsContact {
        email: event.data.email.clone(),
        source: "app_signup".to_string(),
    };
    let loops_api_key = app_config.loops_api_key.clone();
    let url = "https://app.loops.so/api/v1/contacts/create".to_string();

    let send_future = async move {
        let response = Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", loops_api_key))
            .header("Content-Type", "application/json")
            .json(&loops_contact)
            .send()
            .await;

        match response {
            Ok(response) => {
                debug!("Loops response: {:?}", response);
            }
            Err(e) => {
                error!("Failed to send user invite to Loops: {:?}", e);
            }
        }
    };

    // Spawn a new task to send the request to Loops asynchronously
    actix_web::rt::spawn(send_future);

    Ok(HttpResponse::Ok().finish())
}
