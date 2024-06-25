use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema, Debug)]
pub struct AuthCallbackQuery {
    pub code: String,
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
pub struct WorkOSUser {
    pub object: String,
    pub id: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email_verified: bool,
    pub profile_picture_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// For the request payload
#[derive(Serialize, ToSchema, Debug)]
pub struct WorkOSAuthRequest {
    pub client_id: String,
    pub client_secret: String,
    pub grant_type: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invitation_code: Option<String>,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct WorkOSAuthResponse {
    pub user: WorkOSUser,
    #[allow(dead_code)] // We never really use organization_id but whatever
    pub organization_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WorkOSCreateUserWebhookPayload {
    pub id: String,
    pub event: String,
    pub data: WorkOSUser,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListMetadata {
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct GetUserResponse {
    pub data: Vec<WorkOSUser>,
    pub list_metadata: ListMetadata,
}
