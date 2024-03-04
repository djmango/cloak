use actix_web::{
    dev::Payload, error::ResponseError, http::StatusCode, Error, FromRequest, HttpRequest,
    HttpResponse,
};
use futures::future::{ready, Ready};
use jsonwebtoken::{decode, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;

#[derive(Clone)]
pub struct JWTKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JWTKeys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Debug)]
pub enum AuthError {
    InvalidToken,
    WrongCredentials,
    TokenCreation,
    MissingCredentials,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthError::InvalidToken => write!(f, "Invalid token"),
            AuthError::WrongCredentials => write!(f, "Wrong credentials"),
            AuthError::TokenCreation => write!(f, "Token creation error"),
            AuthError::MissingCredentials => write!(f, "Missing credentials"),
        }
    }
}

// Implementing std::error::Error trait for AuthError.
impl std::error::Error for AuthError {}

impl ResponseError for AuthError {
    // Optionally implement the `error_response()` method directly
    // Or implement `status_code()` and `error_response()` separately for more control
    fn error_response(&self) -> HttpResponse {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
        };

        let body = json!({
            "error": error_message,
        });

        HttpResponse::build(status).json(body)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    username: String,
    exp: usize,
}

// impl FromRequest for Claims {
//     type Error = Error;
//     type Future = Ready<Result<Claims, Error>>;

//     fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
//         // This assumes you have some struct KEYS in scope with decoding info
//         // Replace `your_decoding_key_here` with actual logic to retrieve the key
//         let decoding_key = DecodingKey::from_secret("your_decoding_key_here".as_ref());

//         match req.headers().get("Authorization") {
//             Some(header_value) => {
//                 if let Ok(token) = header_value.to_str() {
//                     let token_data = decode::<Claims>(
//                         &token.replace("Bearer ", ""), // Remove Bearer prefix
//                         &decoding_key,
//                         &Validation::default(),
//                     );
//                     match token_data {
//                         Ok(data) => ready(Ok(data.claims)),
//                         Err(_err) => {
//                             ready(Err(actix_web::error::ErrorUnauthorized("Invalid token")))
//                         }
//                     }
//                 } else {
//                     ready(Err(actix_web::error::ErrorUnauthorized("Bad token")))
//                 }
//             }
//             None => ready(Err(actix_web::error::ErrorUnauthorized("Missing token"))),
//         }
//     }
// }
// use actix_service::{Service, Transform};
// use actix_web::dev::{ServiceRequest, ServiceResponse};
// use futures::future::ok;
// use std::rc::Rc;
// use std::task::{Context, Poll};

// pub struct AuthMiddleware {
//     pub config: Rc<AppConfig>,
// }

// // Middleware factory is `Transform` trait from actix-service crate
// // S is the type of the next service
// // B is the type of response's body
// impl<S, B> Transform<S> for AuthMiddleware
// where
//     S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
//     S::Future: 'static,
//     B: 'static,
// {
//     type Request = ServiceRequest;
//     type Response = ServiceResponse<B>;
//     type Error = Error;
//     type InitError = ();
//     type Transform = AuthMiddlewareService<S>;
//     type Future = Ready<Result<Self::Transform, Self::InitError>>;

//     fn new_transform(&self, service: S) -> Self::Future {
//         ok(AuthMiddlewareService {
//             config: self.config.clone(),
//             service: Rc::new(service),
//         })
//     }
// }

// pub struct AuthMiddlewareService<S> {
//     config: Rc<AppConfig>,
//     service: Rc<S>,
// }

// impl<S, B> Service for AuthMiddlewareService<S>
// where
//     S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
//     S::Future: 'static,
//     B: 'static,
// {
//     type Request = ServiceRequest;
//     type Response = ServiceResponse<B>;
//     type Error = Error;
//     type Future = futures::future::LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

//     fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
//         self.service.poll_ready(ctx)
//     }

//     fn call(&self, req: ServiceRequest) -> Self::Future {
//         // Here you would extract the Authorization header, decode the JWT,
//         // and validate it using your `self.config.jwt_decoding_key`
//         // For now, let's just forward all requests without doing anything

//         let fut = self.service.call(req);
//         Box::pin(async move {
//             let res = fut.await?;
//             Ok(res)
//         })
//     }
// }
