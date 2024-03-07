use actix_web::{get, web, Responder};
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::middleware::auth::AuthenticatedUser;
use crate::AppState;

#[derive(Serialize, Deserialize, Clone)]
struct UserInvite {
    email: String,
    code: String,
    created_at: DateTime<Utc>,
}
