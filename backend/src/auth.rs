use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::http::HeaderMap;
use rand_core::OsRng;
use uuid::Uuid;

use crate::types::AppError;

pub const ADMIN_TOKEN_HEADER: &str = "x-anicargo-admin-token";
pub const DEVICE_ID_HEADER: &str = "x-anicargo-device-id";

#[derive(Debug, Clone)]
pub enum ViewerIdentity {
    Device { id: String },
    User { id: i64, username: String, is_admin: bool },
}

#[derive(Debug, Clone)]
pub struct AdminIdentity {
    pub username: String,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AppError::internal("failed to hash password"))
}

pub fn verify_password(password_hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn generate_token() -> String {
    Uuid::new_v4().simple().to_string()
}

pub fn extract_device_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(DEVICE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn extract_user_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn extract_admin_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(ADMIN_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
