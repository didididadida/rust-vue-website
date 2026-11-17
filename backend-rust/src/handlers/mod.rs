pub mod articles;
pub mod auth;
pub mod comments;
pub mod health;
pub mod register;
pub mod upload;

use actix_files::Files;
use actix_web::{web, HttpResponse};

use crate::models::ApiResponse;
use crate::paths;

/// JSON 请求体上限（文章正文等）
/// 注意：multipart 上传走 PayloadConfig，必须在 service 级单独设置，
/// 否则会被这里 8MB 的默认值卡住（视频上传需要更大的上限）。
pub const JSON_LIMIT: usize = 8 * 1024 * 1024;

/// multipart / 原始请求体上限：视频上传按 220MB 放开（单文件在 handler 里另有 200MB 限制）
pub const PAYLOAD_LIMIT: usize = 220 * 1024 * 1024;

/// 所有路由的集中注册
///
/// 范围：博客 + 评论 + 图床。
/// 不包含论坛、私信、弹幕、项目、工具、垃圾箱——这些功能已从本项目移出。
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.app_data(web::JsonConfig::default().limit(JSON_LIMIT))
        .app_data(web::PayloadConfig::default().limit(PAYLOAD_LIMIT))
        .route("/api/health", web::get().to(health::health_check))
        // ---------- 认证 ----------
        .route("/api/login", web::post().to(auth::login))
        .route("/api/register", web::post().to(register::register))
        .route("/api/register/send-code", web::post().to(register::send_code))
        .route("/api/register/status", web::get().to(register::register_status))
        .route("/api/me", web::get().to(register::me))
        .route("/api/profile", web::put().to(register::update_profile))
        .route("/api/profile/password", web::put().to(register::change_password))
        .route("/api/profile/{username}", web::get().to(register::get_profile))
        // ---------- 博客文章（posts/*.md）----------
        .route("/api/articles", web::get().to(articles::list_articles))
        .route("/api/articles", web::post().to(articles::create_article))
        // 批量删除 —— 必须放在 {slug} 路由之前，否则会被当成 slug
        .route("/api/articles/batch-delete", web::post().to(articles::batch_delete))
        // 审核（管理员）—— 放在 {slug} 路由之前，避免被当成 slug 匹配
        .route("/api/articles/review/pending", web::get().to(articles::list_pending))
        .route("/api/articles/{slug}/review", web::put().to(articles::review_article))
        // 原始 .md 读写（纯文本编辑器）—— 同样必须在 {slug} 之前
        .route("/api/articles/{slug}/raw", web::get().to(articles::get_raw))
        .route("/api/articles/{slug}/raw", web::put().to(articles::put_raw))
        .route("/api/articles/{slug}", web::get().to(articles::get_article))
        .route("/api/articles/{slug}", web::put().to(articles::update_article))
        .route("/api/articles/{slug}", web::delete().to(articles::delete_article))
        // ---------- 评论（data/comments/*.json）----------
        .route("/api/articles/{slug}/comments", web::get().to(comments::list_comments))
        .route("/api/articles/{slug}/comments", web::post().to(comments::create_comment))
        .route("/api/comments", web::get().to(comments::list_all_comments))
        // 批量删除 —— 必须放在 {slug}/{id} 路由之前
        .route("/api/comments/batch-delete", web::post().to(comments::batch_delete))
        .route("/api/comments/{slug}/{id}", web::delete().to(comments::delete_comment))
        .route("/api/comments/{slug}/{id}/approve", web::put().to(comments::approve_comment))
        // ---------- 图床（图片 / 视频共用上传端点，按扩展名区分）----------
        .route("/api/upload", web::post().to(upload::upload_file))
        .route("/api/images", web::get().to(upload::list_images))
        .route("/api/images/{filename}", web::delete().to(upload::delete_image))
        // 图片等静态资源（视频也在同一个上传目录下）
        .service(Files::new("/static", paths::static_dir()))
        // 兜底：统一返回 JSON，避免前端解析 HTML 报错
        .default_service(web::route().to(not_found));
}

pub async fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::error(404, "接口或资源不存在"))
}
