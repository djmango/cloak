use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};
use std::time::Instant;
use tracing::{error, info};

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct LoggingMiddleware;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for LoggingMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = LoggingMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(LoggingMiddlewareService { service }))
    }
}

pub struct LoggingMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for LoggingMiddlewareService<S>
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
        let path = req.path().to_string();
        let method = req.method().to_string();
        let referer = req
            .headers()
            .get("Referer")
            .map(|v| v.to_str().unwrap_or(""))
            .unwrap_or("")
            .to_string();
        let user_agent = req
            .headers()
            .get("User-Agent")
            .map(|v| v.to_str().unwrap_or(""))
            .unwrap_or("")
            .to_string();
        let start_time = Instant::now();

        info!(
            "Request started: {} {} - Referer: {}, User-Agent: {}",
            method, path, referer, user_agent
        );

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;

            let status_code = res.status().as_u16();
            let elapsed = start_time.elapsed();

            let log_message = format!(
                "Request completed: {} {} {} - Time taken: {:?}",
                method, path, status_code, elapsed
            );

            match status_code {
                400..=499 => error!("{}", log_message),
                500..=599 => error!("{}", log_message),
                _ => info!("{}", log_message),
            }

            Ok(res)
        })
    }
}
