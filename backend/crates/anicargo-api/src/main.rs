use anicargo_media::{ensure_hls, find_entry_by_id, scan_media, MediaConfig, MediaError, MediaEntry};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Json;
use axum::Router;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

const DEFAULT_BIND: &str = "0.0.0.0:3000";
const DEFAULT_TOKEN_TTL_SECS: u64 = 3600;
const DEFAULT_ADMIN_USER: &str = "admin";
const DEFAULT_ADMIN_PASSWORD: &str = "adminpwd";
const DEFAULT_INVITE_CODE: &str = "invitecode";

#[derive(Clone)]
struct AppState {
    config: Arc<MediaConfig>,
    db: PgPool,
    auth: Arc<AuthConfig>,
}

#[derive(Debug, Clone)]
struct AuthConfig {
    jwt_secret: String,
    token_ttl: Duration,
    admin_user: String,
    admin_password: String,
    invite_code: String,
}

#[derive(Debug, Serialize)]
struct StreamResponse {
    id: String,
    playlist_url: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    token: String,
    user_id: String,
    role: UserRole,
    expires_in: u64,
}

#[derive(Debug, Serialize)]
struct CreateUserResponse {
    user_id: String,
    role: UserRole,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    user_id: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    user_id: String,
    password: String,
    invite_code: String,
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum UserRole {
    Admin,
    User,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    role: UserRole,
    exp: u64,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<MediaError> for ApiError {
    fn from(err: MediaError) -> Self {
        match err {
            MediaError::NotFound(message) => ApiError {
                status: StatusCode::NOT_FOUND,
                message,
            },
            MediaError::MissingMediaDir => ApiError {
                status: StatusCode::BAD_REQUEST,
                message: err.to_string(),
            },
            MediaError::InvalidMediaDir(_) | MediaError::InvalidConfig(_) => ApiError {
                status: StatusCode::BAD_REQUEST,
                message: err.to_string(),
            },
            MediaError::Io(_) => ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: err.to_string(),
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        (self.status, body).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MediaConfig::from_env()?;
    let hls_root = config.hls_root();
    fs::create_dir_all(&hls_root)?;

    let auth = load_auth_config();
    let db = connect_db().await?;
    init_db(&db).await?;
    ensure_admin(&db, &auth).await?;

    let state = AppState {
        config: Arc::new(config),
        db,
        auth: Arc::new(auth),
    };

    let app = Router::new()
        .route("/api/library", get(library_handler))
        .route("/api/stream/:id", get(stream_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/users", post(create_user_handler))
        .route("/api/users/:id", delete(delete_user_handler))
        .route("/hls/:token/:id/:file", get(hls_file_handler_with_token))
        .route("/hls/:id/:file", get(hls_file_handler))
        .with_state(state);

    let bind_addr = env::var("ANICARGO_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn library_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<Vec<MediaEntry>>, ApiError> {
    require_auth(&state, &headers, &query, None).await?;
    let entries = scan_media(&state.config)?;
    Ok(Json(entries))
}

async fn stream_handler(
    Path(id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<StreamResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query, None).await?;

    let entry = find_entry_by_id(&state.config, &id)?;
    let session = ensure_hls(&entry, &state.config)?;

    let playlist_url = format!("/hls/{}/{}/index.m3u8", auth.token, session.id);

    Ok(Json(StreamResponse { id, playlist_url }))
}

async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let user = fetch_user(&state.db, &payload.user_id).await?;
    verify_password(&payload.password, &user.password_hash)?;

    let token = issue_token(&state.auth, &user.user_id, user.role)?;

    Ok(Json(LoginResponse {
        token,
        user_id: user.user_id,
        role: user.role,
        expires_in: state.auth.token_ttl.as_secs(),
    }))
}

async fn create_user_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<CreateUserResponse>, ApiError> {
    if payload.invite_code != state.auth.invite_code {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "invalid invite code".to_string(),
        });
    }

    let hash = hash_password(&payload.password)?;
    let created = create_user(&state.db, &payload.user_id, &hash, UserRole::User).await?;

    Ok(Json(CreateUserResponse {
        user_id: created.user_id,
        role: created.role,
    }))
}

async fn delete_user_handler(
    Path(id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<StatusCode, ApiError> {
    let auth = require_auth(&state, &headers, &query, None).await?;
    if auth.role != UserRole::Admin && auth.user_id != id {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "forbidden".to_string(),
        });
    }

    delete_user(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn hls_file_handler(
    Path((id, file)): Path<(String, String)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers, &query, None).await?;
    serve_hls_file(&state, &id, &file).await
}

async fn require_auth(
    state: &AppState,
    headers: &HeaderMap,
    query: &Query<TokenQuery>,
    token_override: Option<&str>,
) -> Result<AuthContext, ApiError> {
    let token = if let Some(token) = token_override {
        token.to_string()
    } else {
        extract_token(headers, &query.0).ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "missing token".to_string(),
        })?
    };

    let claims = decode_token(&state.auth, &token)?;

    Ok(AuthContext {
        user_id: claims.sub,
        role: claims.role,
        token,
    })
}

async fn hls_file_handler_with_token(
    Path((token, id, file)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let headers = HeaderMap::new();
    let query = Query(TokenQuery { token: None });
    require_auth(&state, &headers, &query, Some(&token)).await?;
    serve_hls_file(&state, &id, &file).await
}

async fn serve_hls_file(state: &AppState, id: &str, file: &str) -> Result<Response, ApiError> {
    let root = state.config.hls_root();
    let file_path = root.join(id).join(file);
    let safe_path = ensure_within_root(&root, &file_path)?;

    let file = File::open(&safe_path).await.map_err(|_| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "file not found".to_string(),
    })?;

    let content_type = match safe_path.extension().and_then(|ext| ext.to_str()) {
        Some("m3u8") => "application/vnd.apple.mpegurl",
        Some("ts") => "video/mp2t",
        _ => "application/octet-stream",
    };

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    Ok(([(header::CONTENT_TYPE, content_type)], body).into_response())
}
struct AuthContext {
    user_id: String,
    role: UserRole,
    token: String,
}

fn extract_token(headers: &HeaderMap, query: &TokenQuery) -> Option<String> {
    if let Some(token) = &query.token {
        return Some(token.clone());
    }

    let header_value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = header_value.strip_prefix("Bearer ")?;
    Some(token.to_string())
}

fn load_auth_config() -> AuthConfig {
    let admin_user = env::var("ANICARGO_ADMIN_USER").unwrap_or_else(|_| DEFAULT_ADMIN_USER.to_string());
    let admin_password =
        env::var("ANICARGO_ADMIN_PASSWORD").unwrap_or_else(|_| DEFAULT_ADMIN_PASSWORD.to_string());
    let invite_code = env::var("ANICARGO_INVITE_CODE").unwrap_or_else(|_| DEFAULT_INVITE_CODE.to_string());
    let jwt_secret = env::var("ANICARGO_JWT_SECRET").unwrap_or_else(|_| "dev-secret".to_string());
    let token_ttl = env::var("ANICARGO_TOKEN_TTL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TOKEN_TTL_SECS);

    AuthConfig {
        jwt_secret,
        token_ttl: Duration::from_secs(token_ttl),
        admin_user,
        admin_password,
        invite_code,
    }
}

async fn connect_db() -> Result<PgPool, ApiError> {
    let url = env::var("ANICARGO_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "missing ANICARGO_DATABASE_URL".to_string(),
        })?;

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("database connection failed: {}", err),
        })
}

async fn init_db(db: &PgPool) -> Result<(), ApiError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (\
            id TEXT PRIMARY KEY,\
            password_hash TEXT NOT NULL,\
            role TEXT NOT NULL,\
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init db: {}", err),
    })?;

    Ok(())
}

async fn ensure_admin(db: &PgPool, auth: &AuthConfig) -> Result<(), ApiError> {
    let hash = hash_password(&auth.admin_password)?;

    sqlx::query(
        "INSERT INTO users (id, password_hash, role) VALUES ($1, $2, $3)\
        ON CONFLICT (id) DO NOTHING",
    )
    .bind(&auth.admin_user)
    .bind(&hash)
    .bind("admin")
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to ensure admin user: {}", err),
    })?;

    Ok(())
}

struct DbUser {
    user_id: String,
    password_hash: String,
    role: UserRole,
}

async fn fetch_user(db: &PgPool, user_id: &str) -> Result<DbUser, ApiError> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, password_hash, role FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch user: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "invalid credentials".to_string(),
    })?;

    let role = parse_role(&row.2)?;

    Ok(DbUser {
        user_id: row.0,
        password_hash: row.1,
        role,
    })
}

async fn create_user(
    db: &PgPool,
    user_id: &str,
    password_hash: &str,
    role: UserRole,
) -> Result<DbUser, ApiError> {
    let role_str = role_to_str(role);
    let row = sqlx::query_as::<_, (String, String, String)>(
        "INSERT INTO users (id, password_hash, role) VALUES ($1, $2, $3)\
        RETURNING id, password_hash, role",
    )
    .bind(user_id)
    .bind(password_hash)
    .bind(role_str)
    .fetch_one(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("failed to create user: {}", err),
    })?;

    let role = parse_role(&row.2)?;

    Ok(DbUser {
        user_id: row.0,
        password_hash: row.1,
        role,
    })
}

async fn delete_user(db: &PgPool, user_id: &str) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("failed to delete user: {}", err),
        })?;

    if result.rows_affected() == 0 {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "user not found".to_string(),
        });
    }

    Ok(())
}

fn parse_role(role: &str) -> Result<UserRole, ApiError> {
    match role {
        "admin" => Ok(UserRole::Admin),
        "user" => Ok(UserRole::User),
        _ => Err(ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "invalid role".to_string(),
        }),
    }
}

fn role_to_str(role: UserRole) -> &'static str {
    match role {
        UserRole::Admin => "admin",
        UserRole::User => "user",
    }
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "failed to hash password".to_string(),
        })
}

fn verify_password(password: &str, hash: &str) -> Result<(), ApiError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "invalid password hash".to_string(),
    })?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "invalid credentials".to_string(),
        })
}

fn issue_token(auth: &AuthConfig, user_id: &str, role: UserRole) -> Result<String, ApiError> {
    let exp = SystemTime::now()
        .checked_add(auth.token_ttl)
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .ok_or_else(|| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "failed to build token".to_string(),
        })?;

    let claims = Claims {
        sub: user_id.to_string(),
        role,
        exp,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(auth.jwt_secret.as_bytes()),
    )
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "failed to issue token".to_string(),
    })
}

fn decode_token(auth: &AuthConfig, token: &str) -> Result<Claims, ApiError> {
    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(auth.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "invalid token".to_string(),
    })
}

fn ensure_within_root(root: &StdPath, path: &StdPath) -> Result<PathBuf, ApiError> {
    let root = root
        .canonicalize()
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "invalid hls root".to_string(),
        })?;
    let candidate = path
        .canonicalize()
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "file not found".to_string(),
        })?;

    if candidate.starts_with(&root) {
        Ok(candidate)
    } else {
        Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "invalid path".to_string(),
        })
    }
}
