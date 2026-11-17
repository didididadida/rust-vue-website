use actix_web::HttpResponse;
use chrono::Utc;

use crate::comments;
use crate::content::{self, ArticleRecord};
use crate::errors::AppError;
use crate::models::{ApiResponse, HealthResponse, StorageResponse};
use crate::paths;

/// GET /api/health —— 顺带报告文章与评论数量
pub async fn health_check() -> Result<HttpResponse, AppError> {
    let resp = HealthResponse {
        status: "ok".to_string(),
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        version: env!("CARGO_PKG_VERSION").to_string(),
        storage: StorageResponse {
            root: paths::root().display().to_string(),
            articles: content::list::<ArticleRecord>(&paths::posts_dir())?.len(),
            comments: comments::count_all(),
        },
    };
    Ok(HttpResponse::Ok().json(ApiResponse::success(resp)))
}
