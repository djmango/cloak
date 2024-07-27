use actix_web::{get, post, web};
use anyhow::Result;
use uuid::Uuid;
use std::sync::Arc;
use tracing::error;

use crate::models::Devent;
use crate::types::CreateDeventRequest;
use crate::{middleware::auth::AuthenticatedUser, AppState};

#[post("/create")]
async fn create_devent(
    app_state: web::Data<Arc<AppState>>,
    _authenticated_user: AuthenticatedUser,
    req_body: web::Json<CreateDeventRequest>,    
) -> Result<web::Json<Devent>, actix_web::Error> {
    let devent = Devent::new(
        &app_state.pool,
        req_body.session_id,
        req_body.recording_id,
        req_body.mouse_action.clone(), 
        req_body.keyboard_action.clone(), 
        req_body.scroll_action.clone(), 
        req_body.mouse_x, 
        req_body.mouse_y, 
        req_body.event_timestamp_nanos
    )
    .await
    .map_err(|e|{
        error!("Error creating devent: {:?}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    Ok(web::Json(devent))
}

#[get("/{id}")]
async fn get_devent(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    id: web::Path<Uuid>,
) -> Result<web::Json<Devent>, actix_web::Error> {
    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized(
            "Unauthorized".to_string(),
        ));
    }

    let devent = Devent::get(&app_state.pool, id.into_inner())
        .await
        .map_err(|e|{
            error!("Error getting devent: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(web::Json(devent))
}

#[get("/session/{session_id}")]
async fn get_devents_for_session(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    session_id: web::Path<Uuid>,
) -> Result<web::Json<Vec<Devent>>, actix_web::Error> {
    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized(
            "Unauthorized".to_string(),
        ));
    }

    let devents = Devent::get_all_for_session(&app_state.pool, session_id.into_inner())
        .await
        .map_err(|e|{
            error!("Error getting devents: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(web::Json(devents))
}

#[get("/recording/{recording_id}")]
async fn get_devents_for_recording(
    app_state: web::Data<Arc<AppState>>,
    authenticated_user: AuthenticatedUser,
    recording_id: web::Path<Uuid>,
) -> Result<web::Json<Vec<Devent>>, actix_web::Error> {
    if !authenticated_user.is_admin() {
        return Err(actix_web::error::ErrorUnauthorized(
            "Unauthorized".to_string(),
        ));
    }

    let devents = Devent::get_all_for_recording(&app_state.pool, recording_id.into_inner())
        .await
        .map_err(|e|{
            error!("Error getting devents: {:?}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    Ok(web::Json(devents))
}