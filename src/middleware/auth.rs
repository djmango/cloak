use std::{
    future::{ready, Ready},
    sync::Arc,
};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header::AUTHORIZATION,
    Error, FromRequest, HttpMessage, HttpRequest,
};
use futures_util::future::LocalBoxFuture;
use jsonwebtoken::{decode, DecodingKey, Validation};
use tracing::{info, warn};

use crate::{routes::auth::Claims, AppConfig};

#[derive(Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
}

// This is the trait that actix-web uses to extract the `AuthenticatedUser` from the request
// This is how we can use `AuthenticatedUser` as a parameter in our route handlers
// It automatically returns a 401 Unauthorized if the user is not authenticated
impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Ready<Result<AuthenticatedUser, Error>>;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        if let Some(auth_user) = req.extensions().get::<AuthenticatedUser>() {
            ready(Ok(auth_user.clone())) // Assuming `AuthenticatedUser` can be cheaply cloned
        } else {
            ready(Err(ErrorUnauthorized("User not authenticated")))
        }
    }
}

pub struct Authentication {
    pub app_config: Arc<AppConfig>,
}

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for Authentication
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthenticationMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthenticationMiddleware {
            service,
            app_config: self.app_config.clone(),
        }))
    }
}

pub struct AuthenticationMiddleware<S> {
    service: S,
    app_config: Arc<AppConfig>,
}

impl<S, B> Service<ServiceRequest> for AuthenticationMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Here's where we extract JWT from the request, validate it, and insert the user_id into the request extensions
        let app_config = self.app_config.clone();

        let auth_header = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.starts_with("Bearer "))
            .map(|value| &value["Bearer ".len()..]);

        match auth_header {
            Some(token) => {
                let decoding_key = DecodingKey::from_secret(app_config.jwt_secret.as_ref());

                match decode::<Claims>(token, &decoding_key, &Validation::default()) {
                    Ok(token_data) => {
                        let claims = token_data.claims;
                        let user_id = claims.sub;

                        info!("Authenticated user: {}", &user_id);
                        req.extensions_mut().insert(AuthenticatedUser { user_id });
                    }
                    Err(e) => {
                        warn!("Invalid token: {:?}", e);
                    }
                }
            }
            None => {
                info!("No Authorization header found.");
            }
        };

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}