use actix_web::{get, web, Responder};
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use stripe::generated::checkout::checkout_session;
use stripe::{
    CheckoutSessionMode, CreateCheckoutSession, CreateCheckoutSessionDiscounts,
    CreateCheckoutSessionLineItems, CustomerSearchParams, ListPromotionCodes, ListSubscriptions,
    SubscriptionStatusFilter,
};
use tracing::{error, info, warn};

use crate::middleware::auth::AuthenticatedUser;
use crate::routes::auth::user_id_to_user;
use crate::{AppConfig, AppState};

#[derive(Serialize, Deserialize, Clone)]
struct UserInvite {
    email: String,
    code: String,
    created_at: Option<DateTime<Utc>>,
}

#[get("/invite")]
async fn invite(
    app_state: web::Data<AppState>,
    query: web::Query<UserInvite>,
) -> Result<impl Responder, actix_web::Error> {
    let mut user_invite = query.into_inner();
    user_invite.created_at = Utc::now().into();

    // Store the user invite data in Shuttle Persist
    let result = app_state
        .persist
        // .save(&user_invite.email, &user_invite)
        .save::<UserInvite>(
            &format!("user_invite:{}", &user_invite.email),
            user_invite.clone(),
        )
        .map_err(|e| anyhow!("Failed to store user invite: {:?}", e));

    match result {
        Ok(_) => {
            info!("User invite stored successfully: {:?}", user_invite.email);
            Ok(web::Redirect::to("https://github.com/InvisibilityInc/Invisibility/releases/download/2.0.0/Invisibility.Installer.2.0.0.dmg"))
        }
        Err(e) => {
            error!("Failed to store user invite: {:?}", e);
            Err(actix_web::error::ErrorInternalServerError(e.to_string()))
        }
    }
}

#[get("/payment_success")]
async fn payment_success() -> Result<impl Responder, actix_web::Error> {
    Ok(web::Redirect::to("invisibility://paid"))
}

#[derive(Serialize, Deserialize, Clone)]
struct CheckoutRequest {
    email: String,
}

#[get("/checkout")]
async fn checkout(
    app_state: web::Data<AppState>,
    query: web::Query<CheckoutRequest>,
) -> Result<impl Responder, actix_web::Error> {
    info!("Checkout request");
    let checkout_request = query.into_inner();
    info!("Checkout request for email: {}", checkout_request.email);

    // Price is hardcoded
    let line_item = CreateCheckoutSessionLineItems {
        price: Some("price_1Or7FsHQqwgWa5gA8e1L5wna".into()),
        quantity: Some(1),
        ..Default::default()
    };

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

            // Search for the promotion code by listing all active promotion codes
            let promotion_codes = stripe::PromotionCode::list(
                &app_state.stripe_client,
                &ListPromotionCodes {
                    code: Some(user_invite.code.as_str()),
                    active: Some(true),
                    ..Default::default()
                },
            )
            .await;

            match promotion_codes {
                Ok(promotion_codes) => {
                    if let Some(promotion_code) = promotion_codes.data.first() {
                        info!("Promotion code found: {:?}", promotion_code);
                        Some(vec![CreateCheckoutSessionDiscounts {
                            promotion_code: Some(promotion_code.id.as_str().into()),
                            ..Default::default()
                        }])
                    } else {
                        warn!("Promotion code not found for code: {}", user_invite.code);
                        None
                    }
                }
                Err(e) => {
                    error!("Failed to list promotion codes: {:?}", e);
                    None
                }
            }
        }
        _ => {
            warn!(
                "User invite not found for email: {}",
                checkout_request.email
            );
            None
        }
    };

    // Create the checkout session
    // If discounts are found, apply them
    // If no discounts are found, create a checkout session without discounts but allow promotion codes
    let create_checkout_sesssion: CreateCheckoutSession = match discounts {
        Some(discounts) => {
            info!("Discounts applied: {:?}", discounts);
            CreateCheckoutSession {
                customer_email: checkout_request.email.as_str().into(),
                discounts: discounts.into(),
                line_items: vec![line_item].into(),
                mode: CheckoutSessionMode::Subscription.into(),
                success_url: "https://cloak.invisibility.so/pay/payment_success".into(),
                ..Default::default()
            }
        }
        None => {
            info!("No discounts applied");
            CreateCheckoutSession {
                allow_promotion_codes: Some(true),
                customer_email: checkout_request.email.as_str().into(),
                line_items: vec![line_item].into(),
                mode: CheckoutSessionMode::Subscription.into(),
                success_url: "https://cloak.invisibility.so/pay/payment_success".into(),
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
    app_state: web::Data<AppState>,
    app_config: web::Data<Arc<AppConfig>>,
    authenticated_user: AuthenticatedUser,
) -> Result<impl Responder, actix_web::Error> {
    let user = user_id_to_user(&authenticated_user.user_id, app_config.get_ref().clone())
        .await
        .map_err(|e| actix_web::error::ErrorForbidden(e.to_string()))?;

    info!("Paid request for email: {}", user.email);

    // Search for the customer by email using the Stripe API
    let customers = stripe::Customer::search(
        &app_state.stripe_client,
        CustomerSearchParams {
            query: format!("email:'{}'", user.email),
            ..Default::default()
        },
    )
    .await;

    match customers {
        Ok(customers) => {
            if let Some(customer) = customers.data.first() {
                info!("Customer found: {:?}", customer);

                // Retrieve the customer's subscriptions
                let subscriptions = stripe::Subscription::list(
                    &app_state.stripe_client,
                    &ListSubscriptions {
                        customer: Some(customer.id.clone()),
                        status: Some(SubscriptionStatusFilter::Active),
                        ..Default::default()
                    },
                )
                .await;

                match subscriptions {
                    Ok(subscriptions) => {
                        if !subscriptions.data.is_empty() {
                            info!("Active subscription found for customer: {:?}", customer);
                            Ok("You have an active subscription")
                        } else {
                            warn!("No active subscription found for customer: {:?}", customer);
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
            } else {
                warn!("Customer not found for email: {}", user.email);
                Err(actix_web::error::ErrorNotFound("Customer not found"))
            }
        }
        Err(e) => {
            error!("Failed to search for customer: {:?}", e);
            Err(actix_web::error::ErrorInternalServerError(e.to_string()))
        }
    }
}
