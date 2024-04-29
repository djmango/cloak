use actix_web::web::Json;
use actix_web::{get, web, Responder};
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use stripe::generated::checkout::checkout_session;
use stripe::{
    BillingPortalSession, CheckoutSession, CheckoutSessionId, CheckoutSessionMode,
    CreateBillingPortalSession, CreateCheckoutSession, CreateCheckoutSessionDiscounts,
    CreateCheckoutSessionLineItems, Customer, CustomerSearchParams, ListSubscriptions,
    UpdateCustomer,
};
use tracing::{error, info, warn};

use crate::middleware::auth::AuthenticatedUser;
use crate::routes::auth::{user_email_to_user, user_id_to_user};
use crate::{AppConfig, AppState};

#[derive(Serialize, Deserialize, Clone)]
struct UserInvite {
    email: String,
    code: String,
    created_at: Option<DateTime<Utc>>,
}

#[get("/invite")]
async fn invite(
    app_state: web::Data<Arc<AppState>>,
    query: web::Query<UserInvite>,
) -> Result<impl Responder, actix_web::Error> {
    let mut user_invite = query.into_inner();
    user_invite.created_at = Utc::now().into();

    // Store the user invite data in Shuttle Persist
    let result = app_state
        .persist
        .save::<UserInvite>(
            &format!("user_invite:{}", &user_invite.email),
            user_invite.clone(),
        )
        .map_err(|e| anyhow!("Failed to store user invite: {:?}", e));

    match result {
        Ok(_) => {
            info!("User invite stored successfully: {:?}", user_invite.email);
            Ok("User invite stored successfully")
        }
        Err(e) => {
            error!("Failed to store user invite: {:?}", e);
            Err(actix_web::error::ErrorInternalServerError(e.to_string()))
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct PaymentSuccessRequest {
    session_id: String,
    user_email: String,
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

#[derive(Serialize, Deserialize, Clone)]
struct CheckoutRequest {
    email: String,
}

#[get("/checkout")]
async fn checkout(
    app_state: web::Data<Arc<AppState>>,
    query: web::Query<CheckoutRequest>,
) -> Result<impl Responder, actix_web::Error> {
    let checkout_request = query.into_inner();
    info!("Checkout request for email: {}", checkout_request.email);

    // Price is hardcoded
    let line_item = CreateCheckoutSessionLineItems {
        price: Some("price_1OsHQoHQqwgWa5gAAEIA1AMu".into()),
        quantity: Some(1),
        ..Default::default()
    };

    // If a user invite is found, search for the promotion code and retrieve its ID
    let subscription_data = stripe::CreateCheckoutSessionSubscriptionData {
        trial_period_days: Some(10),
        ..Default::default()
    };

    // Success URL is hardcoded, session id is provided by Stripe and user email is passed as a query parameter for matching
    let user_email = checkout_request.email.as_str();
    let success_url = format!(
        "https://cloak.i.inc/pay/payment_success?session_id={{CHECKOUT_SESSION_ID}}&user_email={}",
        user_email
    );

    // Check if a user invite exists for the email
    let user_invite = app_state
        .persist
        .load::<UserInvite>(&format!("user_invite:{}", checkout_request.email));

    // If a user invite is found, search for the promotion code and retrieve its ID
    let discounts: Option<Vec<CreateCheckoutSessionDiscounts>> = match user_invite {
        Ok(user_invite) => {
            info!(
                "User invite found: {:?}, {:?}",
                user_invite.email, user_invite.code
            );

            Some(vec![CreateCheckoutSessionDiscounts {
                coupon: Some("jOQCrk1k".into()),
                ..Default::default()
            }])

            // Search for the promotion code by listing all active promotion codes
        }
        _ => {
            warn!(
                "User invite not found for email: {}",
                checkout_request.email
            );
            None
        }
    };

    // Grab existing users in stripe with the same email, handle gracefully
    let customer =
        get_stripe_user_by_email(app_state.clone(), checkout_request.email.clone()).await;

    // Create the checkout session, taking into account whether the customer already exists
    let create_checkout_sesssion: CreateCheckoutSession = match customer {
        Ok(customer) => {
            info!("Existing customer found: {:?}", customer.email);

            CreateCheckoutSession {
                customer: Some(customer.id.clone()),
                discounts,
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
                discounts,
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

#[derive(Serialize, Deserialize, Clone)]
struct ManageResponse {
    url: String,
}

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
