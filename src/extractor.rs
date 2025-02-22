use crate::store::RateLimiter;
use crate::store::RequestIdentifier;
use crate::store::StoreProcessResult;

use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use axum::extract::OriginalUri;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;

use reqwest::Client;

use serde_json::json;

async fn get_request_path(OriginalUri(o_uri): OriginalUri) -> (String, String) {
    let auth = if let Some(auth) = o_uri.authority() {
        auth.to_string()
    } else {
        String::new()
    };
    (auth, o_uri.path().to_string())
}

async fn to_request_identifier(
    headers: HeaderMap,
    connect: Option<ConnectInfo<SocketAddr>>,
) -> RequestIdentifier {
    if let Some(auth) = headers
        .get("Authorization")
        .and_then(|auth| auth.to_str().ok())
    {
        if auth.starts_with("Bearer ") {
            let token = auth.trim_start_matches("Bearer ").to_string();
            return RequestIdentifier::Token(token);
        } else {
            return RequestIdentifier::Token(auth.to_string());
        }
    }

    if let Some(ip) = headers
        .get("X-Forwarded-For")
        .and_then(|ip| ip.to_str().ok())
        .and_then(|ip_str| ip_str.split(",").next())
        .and_then(|first| first.trim().parse().ok())
    {
        return RequestIdentifier::Ip(ip);
    }

    connect
        .map(|ConnectInfo(addr)| RequestIdentifier::Ip(addr.ip()))
        .unwrap_or(RequestIdentifier::NoParse)
}

async fn handler(
    headers: HeaderMap,
    connect: Option<ConnectInfo<SocketAddr>>,
    OriginalUri(o_uri): OriginalUri,
    State(rlim): State<RateLimiter>,
) -> Result<(), Response> {
    let ri = to_request_identifier(headers, connect).await;
    if let RequestIdentifier::NoParse = ri {
        let err_msg = json!({
            "error": "Could not identify request origin",
            "message": "No valid authorization token or IP address found"
        });
        let code = StatusCode::BAD_REQUEST;
        return Err((code, Json(err_msg)).into_response());
    }
    let request_path = get_request_path(OriginalUri(o_uri)).await;
    let mut g = rlim.lock().await;
    let flag = g.process(request_path, ri);
    drop(g);
    match flag {
        StoreProcessResult::NotFound => {
            let err_msg = json!({
               "error": "Requested resource was not found on this server",
               "message": "The requested resource of route was not found",
            });
            let code = StatusCode::NOT_FOUND;
            return Err((code, Json(err_msg)).into_response());
        }
        StoreProcessResult::RateLimitExceeded => {
            let err_msg = json!({
                "error": "The rate limit has been exceeded",
                "message": "The rate limit has been exceeded for the request identifier",
            });
            let code = StatusCode::TOO_MANY_REQUESTS;
            return Err((code, Json(err_msg)).into_response());
        }
        StoreProcessResult::Continue => {}
    };
    //use a client to forward the request to the service

    Ok(())
}
