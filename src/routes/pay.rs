use crate::models::Invite;
use actix_web::web::Json;
use actix_web::{get, web, Responder};
use anyhow::anyhow;
use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use stripe::generated::checkout::checkout_session;
use stripe::{
    BillingPortalSession, CheckoutSession, CheckoutSessionId, CheckoutSessionMode,
    CreateBillingPortalSession, CreateCheckoutSession, CreateCheckoutSessionLineItems, Customer,
    CustomerSearchParams, ListSubscriptions, UpdateCustomer,
};
use tracing::{error, info, warn};
use utoipa::OpenApi;

use crate::middleware::auth::AuthenticatedUser;
use crate::routes::auth::{user_email_to_user, user_id_to_user};
use crate::types::{
    CheckoutRequest, InviteQuery, LoopsContact, ManageResponse, PaymentSuccessRequest,
    UserInviteQuery,
};
use crate::{AppConfig, AppState};

#[derive(OpenApi)]
#[openapi(
    paths(get_invite, checkout, paid, manage),
    components(schemas(CheckoutRequest, InviteQuery, ManageResponse, UserInviteQuery))
)]
pub struct ApiDoc;

/// Create an invite for a user given their email and a promotion code
#[utoipa::path(
    get,
    responses((status = 200, description = "User invite stored successfully", body = String, content_type = "text/plain"))
)]
#[get("/invite")]
async fn get_invite(
    app_state: web::Data<Arc<AppState>>,
    app_config: web::Data<Arc<AppConfig>>,
    query: web::Query<UserInviteQuery>,
) -> Result<impl Responder, actix_web::Error> {
    let mut user_invite = query.into_inner();
    user_invite.created_at = Utc::now().into();

    // Store the user invite data in the database
    let invite = Invite::create_invite(
        &app_state.pool,
        &user_invite.email,
        &user_invite.code,
        &app_state.invite_cache,
    )
    .await
    .map_err(|e| {
        error!("Failed to create invite: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;
    info!("User invite stored successfully: {:?}", invite.email);

    // Send the user invite data to Loops asynchronously
    let loops_contact = LoopsContact {
        email: user_invite.email.clone(),
        source: "invite".to_string(),
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
                info!("Loops response: {:?}", response);
            }
            Err(e) => {
                error!("Failed to send user invite to Loops: {:?}", e);
            }
        }
    };

    // Spawn a new task to send the request to Loops asynchronously
    actix_web::rt::spawn(send_future);

    Ok("User invite stored successfully")
}

/// List all user invites or filter by a promotion code
#[utoipa::path(
    get,
    responses((status = 200, description = "List of user invites", body = Vec<Invite>, content_type = "application/json"))
)]
#[get("/list_invites")]
async fn list_invites(
    app_state: web::Data<Arc<AppState>>,
    query: web::Query<InviteQuery>,
) -> Result<impl Responder, actix_web::Error> {
    if let Some(code) = &query.code {
        // Get invites for specific code
        let invites = Invite::get_invites_by_code(&app_state.pool, code)
            .await
            .map_err(|e| {
                error!("Failed to get invites for code {}: {:?}", code, e);
                actix_web::error::ErrorInternalServerError(e)
            })?;
        Ok(web::Json(invites))
    } else {
        // If neither email nor code provided, return error
        Err(actix_web::error::ErrorBadRequest("Invite code is required"))
    }
}

#[get("/payment_success")]
async fn payment_success(
    app_state: web::Data<Arc<AppState>>,
    app_config: web::Data<Arc<AppConfig>>,
    query: web::Query<PaymentSuccessRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let session_query = query.into_inner();

    let session = CheckoutSession::retrieve(
        &app_state.stripe_client,
        &CheckoutSessionId::from_str(&session_query.session_id).unwrap(),
        &["customer"],
    )
    .await;

    match session {
        Ok(session) => {
            info!(
                "Checkout session retrieved for: {:?}",
                session.customer_email
            );
            let workos_user =
                user_email_to_user(&session_query.user_email, app_config.get_ref().clone())
                    .await
                    .map_err(|e| actix_web::error::ErrorForbidden(e.to_string()))?;

            // Set workos_user id as metadata in stripe customer
            let mut metadata: stripe::Metadata = HashMap::new();
            metadata.insert("workos_user_id".to_string(), workos_user.id.to_string());
            info!("Metadata: {:?}", metadata);

            Customer::update(
                &app_state.stripe_client,
                &session.customer.unwrap().id(),
                UpdateCustomer {
                    metadata: Some(metadata),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        }
        Err(e) => {
            error!("Failed to retrieve checkout session: {:?}", e);
            return Err(actix_web::error::ErrorInternalServerError(e.to_string()));
        }
    };

    info!("Payment success for session: {}", session_query.session_id);
    Ok(web::Redirect::to("invisibility://paid"))
}

/// Create a checkout session for a user given their email
#[utoipa::path(
    get,
    request_body = CheckoutRequest,
    responses((status = 200, description = "Checkout session created", body = String, content_type = "text/plain"))
)]
#[get("/checkout")]
async fn checkout(
    app_state: web::Data<Arc<AppState>>,
    query: web::Query<CheckoutRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let checkout_request = query.into_inner();
    info!("Checkout request for email: {}", checkout_request.email);

    // Price is hardcoded
    let line_item = CreateCheckoutSessionLineItems {
        price: Some("price_1P7M3gHQqwgWa5gANnfRYvQM".into()),
        quantity: Some(1),
        ..Default::default()
    };

    // If a user invite is found, search for the promotion code and retrieve its ID
    let subscription_data = stripe::CreateCheckoutSessionSubscriptionData {
        trial_period_days: Some(7),
        ..Default::default()
    };

    // Success URL is hardcoded, session id is provided by Stripe and user email is passed as a query parameter for matching
    let user_email = checkout_request.email.as_str();
    let success_url = format!(
        "https://cloak.i.inc/pay/payment_success?session_id={{CHECKOUT_SESSION_ID}}&user_email={}",
        user_email
    );

    // Check if a user invite exists for the email
    // let user_invite = app_state
    //     .persist
    //     .load::<UserInvite>(&format!("user_invite:{}", checkout_request.email));

    // If a user invite is found, search for the promotion code and retrieve its ID
    // let discounts: Option<Vec<CreateCheckoutSessionDiscounts>> = match user_invite {
    //     Ok(user_invite) => {
    //         info!(
    //             "User invite found: {:?}, {:?}",
    //             user_invite.email, user_invite.code
    //         );

    //         Some(vec![
    //             // $10 off once
    //             CreateCheckoutSessionDiscounts {
    //                 coupon: Some("jOQCrk1k".into()),
    //                 ..Default::default()
    //             },
    //             // 10$ off every month
    //             CreateCheckoutSessionDiscounts {
    //                 coupon: Some("1UqUrexm".into()),
    //                 ..Default::default()
    //             },
    //         ])
    //     }
    //     _ => {
    //         warn!(
    //             "User invite not found for email: {}",
    //             checkout_request.email
    //         );
    //         // None
    //         // Give a default discount
    //         Some(vec![CreateCheckoutSessionDiscounts {
    //             coupon: Some("1UqUrexm".into()),
    //             ..Default::default()
    //         }])
    //     }
    // };

    // let discounts = Some(vec![CreateCheckoutSessionDiscounts {
    //     coupon: Some("1UqUrexm".into()),
    //     ..Default::default()
    // }]);

    // Grab existing users in stripe with the same email, handle gracefully
    let customer =
        get_stripe_user_by_email(app_state.clone(), checkout_request.email.clone()).await;

    // Create the checkout session, taking into account whether the customer already exists
    let create_checkout_sesssion: CreateCheckoutSession = match customer {
        Ok(customer) => {
            info!("Existing customer found: {:?}", customer.email);

            CreateCheckoutSession {
                customer: Some(customer.id.clone()),
                // discounts,
                allow_promotion_codes: Some(true),
                line_items: vec![line_item].into(),
                mode: CheckoutSessionMode::Subscription.into(),
                subscription_data: Some(subscription_data),
                success_url: Some(&success_url),
                ..Default::default()
            }
        }
        Err(e) => {
            info!("Did not find existing customer: {:?}", e);
            CreateCheckoutSession {
                customer_email: checkout_request.email.as_str().into(),
                // discounts,
                allow_promotion_codes: Some(true),
                line_items: vec![line_item].into(),
                mode: CheckoutSessionMode::Subscription.into(),
                subscription_data: Some(subscription_data),
                success_url: Some(&success_url),
                ..Default::default()
            }
        }
    };

    let checkout = checkout_session::CheckoutSession::create(
        &app_state.stripe_client,
        create_checkout_sesssion,
    )
    .await;

    match checkout {
        Ok(checkout) => {
            // Redirect to the checkout URL
            info!("Created checkout session for: {}", checkout_request.email);
            Ok(web::Redirect::to(checkout.url.unwrap()))
        }
        Err(e) => {
            error!("Failed to create checkout session: {:?}", e);
            Err(actix_web::error::ErrorInternalServerError(e.to_string()))
        }
    }
}

/// Check if a user has an active subscription
#[utoipa::path(
    get,
    responses((status = 200, description = "User has an active subscription", body = String, content_type = "text/plain")),
    responses((status = 402, description = "User does not have an active subscription", body = String, content_type = "text/plain"))
)]
#[get("/paid")]
async fn paid(
    app_state: web::Data<Arc<AppState>>,
    app_config: web::Data<Arc<AppConfig>>,
    authenticated_user: AuthenticatedUser,
) -> Result<impl Responder, actix_web::Error> {
    let user = user_id_to_user(&authenticated_user.user_id, app_config.get_ref().clone())
        .await
        .map_err(|e| actix_web::error::ErrorForbidden(e.to_string()))?;

    info!("Paid request for email: {}", user.email);

    // Search for the customer by id or email using the Stripe API
    let customer = get_customer_by_workos_user_id_or_email(
        app_state.clone(),
        user.id.clone(),
        user.email.clone(),
    )
    .await
    .map_err(|e| actix_web::error::ErrorNotFound(e.to_string()))?;

    // Retrieve the customer's subscriptions
    let subscriptions = stripe::Subscription::list(
        &app_state.stripe_client,
        &ListSubscriptions {
            customer: Some(customer.id.clone()),
            ..Default::default()
        },
    )
    .await;

    match subscriptions {
        Ok(subscriptions) => {
            if !subscriptions.data.is_empty() {
                info!(
                    "Active subscription found for customer: {:?}",
                    customer.email
                );
                Ok("You have an active subscription")
            } else {
                warn!(
                    "No active subscription found for customer: {:?}",
                    customer.email
                );
                Err(actix_web::error::ErrorPaymentRequired(
                    "No active subscription found",
                ))
            }
        }
        Err(e) => {
            error!("Failed to retrieve subscriptions: {:?}", e);
            Err(actix_web::error::ErrorInternalServerError(e.to_string()))
        }
    }
}

/// Redirect to the Stripe billing portal for a user
#[utoipa::path(
    get,
    responses((status = 200, description = "Billing portal URL", body = ManageResponse, content_type = "application/json"))
)]
#[get("/manage")]
async fn manage(
    app_state: web::Data<Arc<AppState>>,
    app_config: web::Data<Arc<AppConfig>>,
    authenticated_user: AuthenticatedUser,
) -> Result<Json<ManageResponse>, actix_web::Error> {
    let user = user_id_to_user(&authenticated_user.user_id, app_config.get_ref().clone())
        .await
        .map_err(|e| actix_web::error::ErrorForbidden(e.to_string()))?;

    info!("Manage request for email: {}", user.email);
    // TODO: set actual settings for billing portal, just creates an empty page rn

    // Search for the customer by email using the Stripe API
    let customer = get_stripe_user_by_email(app_state.clone(), user.email.clone())
        .await
        .map_err(|e| actix_web::error::ErrorNotFound(e.to_string()))?;

    let billing_portal = BillingPortalSession::create(
        &app_state.stripe_client,
        CreateBillingPortalSession {
            customer: customer.id.clone(),
            return_url: Some("https://i.inc/"),
            configuration: Default::default(),
            flow_data: Default::default(),
            expand: &[],
            locale: None,
            on_behalf_of: None,
        },
    )
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    Ok(Json(ManageResponse {
        url: billing_portal.url,
    }))
}

async fn get_stripe_user_by_email(
    app_state: web::Data<Arc<AppState>>,
    email: String,
) -> Result<stripe::Customer, anyhow::Error> {
    let customers = Customer::search(
        &app_state.stripe_client,
        CustomerSearchParams {
            query: format!("email:'{}'", email,),
            ..Default::default()
        },
    )
    .await;

    // TODO If we have multiple customers, we should handle that case
    match customers {
        Ok(customers) => {
            if let Some(customer) = customers.data.first() {
                info!("Customer found: {:?}", customer.email);
                Ok(customer.clone())
            } else {
                Err(anyhow!("Customer not found"))
            }
        }
        Err(e) => {
            error!("Failed to search for customer: {:?}", e);
            Err(anyhow!("Failed to search for customer"))
        }
    }
}

async fn get_stripe_user_by_workos_user_id(
    app_state: web::Data<Arc<AppState>>,
    workos_user_id: String,
) -> Result<stripe::Customer, anyhow::Error> {
    let customers = Customer::search(
        &app_state.stripe_client,
        CustomerSearchParams {
            query: format!("metadata['workos_user_id']:'{}'", workos_user_id),
            ..Default::default()
        },
    )
    .await;

    match customers {
        Ok(customers) => {
            if let Some(customer) = customers.data.first() {
                Ok(customer.clone())
            } else {
                Err(anyhow!("Customer not found"))
            }
        }
        Err(e) => {
            error!("Failed to search for customer: {:?}", e);
            Err(anyhow!("Failed to search for customer"))
        }
    }
}

async fn get_customer_by_workos_user_id_or_email(
    app_state: web::Data<Arc<AppState>>,
    workos_user_id: String,
    email: String,
) -> Result<stripe::Customer, anyhow::Error> {
    let customer_by_workos_user_id =
        get_stripe_user_by_workos_user_id(app_state.clone(), workos_user_id.clone()).await;

    match customer_by_workos_user_id {
        Ok(customer) => {
            info!("Customer found by workos_user_id: {:?}", customer.email);
            Ok(customer)
        }
        Err(_) => {
            info!(
                "Customer not found by workos_user_id, trying by email: {}",
                email
            );
            let customer_by_email =
                get_stripe_user_by_email(app_state.clone(), email.clone()).await;
            match customer_by_email {
                Ok(customer) => {
                    info!("Customer found by email: {:?}", customer.email);
                    Ok(customer)
                }
                Err(e) => {
                    warn!(
                        "Failed to find customer by workos_user_id and email: {:?}",
                        e
                    );
                    Err(anyhow!("Customer not found by workos_user_id or email"))
                }
            }
        }
    }
}
