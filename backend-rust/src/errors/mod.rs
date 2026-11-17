use actix_web::{HttpResponse, ResponseError};
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    TooManyRequests(String),
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            AppError::TooManyRequests(msg) => write!(f, "Too Many Requests: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal Error: {}", msg),
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::from_u16(self.status_u16()).unwrap()
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(serde_json::json!({
            "code": self.status_u16(),
            "message": self.message(),
            "data": null
        }))
    }
}

impl AppError {
    pub fn status_u16(&self) -> u16 {
        match self {
            AppError::BadRequest(_) => 400,
            AppError::Unauthorized(_) => 401,
            AppError::Forbidden(_) => 403,
            AppError::NotFound(_) => 404,
            AppError::Conflict(_) => 409,
            AppError::TooManyRequests(_) => 429,
            AppError::Internal(_) => 500,
        }
    }

    pub fn message(&self) -> String {
        match self {
            AppError::BadRequest(m)
            | AppError::Unauthorized(m)
            | AppError::Forbidden(m)
            | AppError::NotFound(m)
            | AppError::Conflict(m)
            | AppError::TooManyRequests(m)
            | AppError::Internal(m) => m.clone(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Internal(format!("文件读写失败: {}", e))
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::Internal(format!("密码处理失败: {}", e))
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        AppError::Unauthorized(format!("JWT 错误: {}", e))
    }
}

impl From<actix_multipart::MultipartError> for AppError {
    fn from(e: actix_multipart::MultipartError) -> Self {
        AppError::BadRequest(format!("上传数据解析失败: {}", e))
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        // 约束冲突单独映射成 409，前端可以据此给出更准确的提示
        match &e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ffi::ErrorCode::ConstraintViolation =>
            {
                AppError::Conflict(format!("数据冲突: {}", e))
            }
            _ => AppError::Internal(format!("数据库错误: {}", e)),
        }
    }
}
