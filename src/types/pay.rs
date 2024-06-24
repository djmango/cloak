use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct UserInvite {
    pub email: String,
    pub code: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LoopsContact {
    pub email: String,
    pub source: String,
}

#[derive(Deserialize)]
pub struct InviteQuery {
    pub code: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentSuccessRequest {
    pub session_id: String,
    pub user_email: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CheckoutRequest {
    pub email: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ManageResponse {
    pub url: String,
}
