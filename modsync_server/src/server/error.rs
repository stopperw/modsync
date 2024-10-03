use axum::{
    extract::multipart::MultipartError,
    response::{IntoResponse, Response},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("database error: {0}")]
    SqlxDatabase(#[from] sqlx::Error),
    #[error("multipart error: {0}")]
    MultipartError(#[from] MultipartError),
    #[error("i/o error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("already exists")]
    AlreadyExists,
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("bad request")]
    BadRequest,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if cfg!(debug_assertions) {
            error!("{:#?}", self);
            (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
        } else {
            match self {
                ApiError::SqlxDatabase(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        error: "DATABASE_ERROR".to_string(),
                    },
                ),
                ApiError::MultipartError(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        error: "MULTIPART_ERROR".to_string(),
                    },
                ),
                ApiError::IoError(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        error: "IO_ERROR".to_string(),
                    },
                ),
                ApiError::AlreadyExists => (
                    StatusCode::BAD_REQUEST,
                    ErrorResponse {
                        error: "ALREADY_EXISTS".to_string(),
                    },
                ),
                ApiError::Unauthorized => (
                    StatusCode::UNAUTHORIZED,
                    ErrorResponse {
                        error: "UNAUTHORIZED".to_string(),
                    },
                ),
                ApiError::NotFound => (
                    StatusCode::NOT_FOUND,
                    ErrorResponse {
                        error: "NOT_FOUND".to_string(),
                    },
                ),
                ApiError::BadRequest => (
                    StatusCode::BAD_REQUEST,
                    ErrorResponse {
                        error: "BAD_REQUEST".to_string(),
                    },
                ),
            }
            .into_response()
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        self.error.into_response()
    }
}

