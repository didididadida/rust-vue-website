//! 健康检查
//!
//! 逻辑描述：
//!   输入 → 输出：无 → { status, timestamp, version, storage: { articles, comments } }
//!   关键机制：
//!     1. 同时报告数据库连通性（查一次计数即可验证库可读）；
//!     2. 数据库不可读时返回错误（而非 200），健康检查必须反映真实状态。
//!   不变量：返回的计数与数据库实际行数一致
//!   边界：
//!     - 该接口不鉴权（供 Nginx / systemd / 云平台探活）；
//!     - 不返回数据目录路径（数据库化后意义下降，且无需暴露）
//!
//! 依据：
//!   经典问题：健康检查与就绪检查（liveness vs readiness）
//!   来源：Kubernetes 官方文档
//!        https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/
//!        要点：探针必须检查"服务真的能工作"（如依赖可读），
//!              只返回 200 而不检查依赖会掩盖故障。
//!   原型结论：
//!     1. 探活接口应验证关键依赖，而不只是进程存活；
//!     2. 依赖不可用时返回非 2xx，让平台能感知。
//!   本处变体：原实现从文件系统数 Markdown 文件；现在改为查数据库计数，
//!             顺便验证数据库可读。
//!   沿用结论：保留接口路径与响应结构（云平台探针依赖 /api/health）。
//!   偏离处：不再返回数据根目录路径。

use actix_web::{web, HttpResponse};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::errors::AppError;
use crate::models::ApiResponse;
use crate::services::{article, comment};

#[derive(Serialize)]
struct StorageInfo {
    articles: u64,
    comments: u64,
}

#[derive(Serialize)]
struct HealthBody {
    status: String,
    timestamp: String,
    version: String,
    storage: StorageInfo,
}

/// GET /api/health
pub async fn health_check(db: web::Data<DatabaseConnection>) -> Result<HttpResponse, AppError> {
    // 查一次计数即可验证数据库可读
    let articles = article::count_all(db.as_ref()).await?;
    let comments = comment::count_all(db.as_ref()).await?;

    let body = HealthBody {
        status: "ok".to_string(),
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        version: env!("CARGO_PKG_VERSION").to_string(),
        storage: StorageInfo { articles, comments },
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(body)))
}
