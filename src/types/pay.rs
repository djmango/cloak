use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct UserInvite {
    pub email: String,
    pub code: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct LoopsContact {
    pub email: String,
    pub source: String,
}

#[derive(Deserialize, ToSchema)]
pub struct InviteQuery {
    pub code: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct PaymentSuccessRequest {
    pub session_id: String,
    pub user_email: String,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct CheckoutRequest {
    pub email: String,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct ManageResponse {
    pub url: String,
}
