use crate::models::user::User;
use crate::routes::auth::WorkOSUser;
use crate::AppState;
use actix_web::HttpResponse;
use actix_web::{
    post,
    web::{self, Json},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

#[derive(Serialize, Deserialize, Debug)]
pub struct WorkOSCreateUserWebhookPayload {
    id: String,
    event: String,
    data: WorkOSUser,
    created_at: DateTime<Utc>,
}

#[post("/workos/create_user")]
pub async fn create_user(
    app_state: web::Data<Arc<AppState>>,
    event: Json<WorkOSCreateUserWebhookPayload>,
) -> Result<HttpResponse, actix_web::Error> {
    let workos_users = vec![event.data.clone()];

    User::get_or_create_or_update_bulk_workos(&app_state.pool, workos_users)
        .await
        .map_err(|err| {
            error!("Error creating user from webhook: {}", err);
            actix_web::error::ErrorInternalServerError("Error creating user from webhook")
        })?;

    Ok(HttpResponse::Ok().finish())
}
