//! 路由注册
//!
//! 逻辑描述：
//!   输入 → 输出：`ServiceConfig` → 完整的路由表
//!   关键机制：
//!     1. 数据库连接与 WebSocket Hub 通过 `web::Data` 注入，跨 worker 共享；
//!     2. 路由顺序敏感：静态路径段必须写在 `{param}` 之前，
//!        否则 `/api/articles/batch-delete` 会被当成 slug；
//!     3. 兜底路由统一返回 JSON，避免前端解析 HTML 报错。
//!   不变量：
//!     - 每个 handler 都能从 `web::Data` 取到连接；
//!     - 通配路由（`{slug}`）之前，所有固定路径都已注册。
//!   边界：
//!     - `/static` 由 actix-files 直接提供，不经鉴权（图片公开可访问）。
//!
//! 依据：
//!   经典问题：路由匹配的优先级（route precedence）
//!   来源：
//!     1. actix-web 官方文档「Resource and route matching」
//!        https://actix.rs/docs/url-dispatch/
//!        要点：路由按注册顺序匹配，因此通配在前会吞掉后面的固定路径。
//!     2. actix-web 关于共享状态的说明
//!        https://actix.rs/docs/application/#shared-mutable-state
//!        要点：HttpServer::new 的闭包每个 worker 执行一次，
//!              闭包内创建的资源不共享，必须用 web::Data 从外部传入。
//!   原型结论：
//!     1. `/api/articles/batch-delete` 必须注册在 `/api/articles/{slug}` 之前；
//!     2. 数据库连接必须在 HttpServer 之前创建。
//!   本处变体：原实现没有数据库，也没有 WebSocket；
//!             现在要注入两个共享状态（DatabaseConnection、Hub）。
//!   沿用结论：保持原有的"固定路径在前"顺序；用 web::Data 注入两个状态。
//!   偏离处：无。

pub mod articles;
pub mod auth;
pub mod comments;
pub mod forum;
pub mod health;
pub mod messages;
pub mod register;
pub mod showcase;
pub mod upload;

use actix_files::Files;
use actix_web::{web, HttpResponse};

use crate::models::ApiResponse;
use crate::paths;

/// JSON 请求体上限（文章正文等）
pub const JSON_LIMIT: usize = 8 * 1024 * 1024;

/// multipart / 原始请求体上限（视频上传）
pub const PAYLOAD_LIMIT: usize = 220 * 1024 * 1024;

/// 所有路由的集中注册
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.app_data(web::JsonConfig::default().limit(JSON_LIMIT))
        .app_data(web::PayloadConfig::default().limit(PAYLOAD_LIMIT))
        // ---------- 健康检查 ----------
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
        // ---------- 文章 ----------
        // 固定路径必须在 {slug} 之前
        .route("/api/articles", web::get().to(articles::list_articles))
        .route("/api/articles", web::post().to(articles::create_article))
        .route("/api/articles/batch-delete", web::post().to(articles::batch_delete))
        .route("/api/articles/review/pending", web::get().to(articles::list_pending))
        .route("/api/articles/{slug}/review", web::put().to(articles::review_article))
        .route("/api/articles/{slug}/raw", web::get().to(articles::get_raw))
        .route("/api/articles/{slug}/raw", web::put().to(articles::put_raw))
        .route("/api/articles/{slug}", web::get().to(articles::get_article))
        .route("/api/articles/{slug}", web::put().to(articles::update_article))
        .route("/api/articles/{slug}", web::delete().to(articles::delete_article))
        // ---------- 评论 ----------
        .route("/api/articles/{slug}/comments", web::get().to(comments::list_comments))
        .route("/api/articles/{slug}/comments", web::post().to(comments::create_comment))
        .route("/api/comments", web::get().to(comments::list_all_comments))
        .route("/api/comments/batch-delete", web::post().to(comments::batch_delete))
        .route("/api/comments/{slug}/{id}", web::delete().to(comments::delete_comment))
        .route("/api/comments/{slug}/{id}/approve", web::put().to(comments::approve_comment))
        // ---------- 论坛 ----------
        .route("/api/forum/boards", web::get().to(forum::list_boards))
        .route("/api/forum/posts", web::get().to(forum::list_posts))
        .route("/api/forum/posts", web::post().to(forum::create_post))
        .route("/api/forum/posts/{id}", web::get().to(forum::get_post))
        .route("/api/forum/posts/{id}", web::put().to(forum::update_post))
        .route("/api/forum/posts/{id}", web::delete().to(forum::delete_post))
        .route("/api/forum/posts/{id}/flags", web::put().to(forum::set_flags))
        .route("/api/forum/posts/{id}/replies", web::post().to(forum::create_reply))
        .route("/api/forum/replies/{id}", web::delete().to(forum::delete_reply))
        // ---------- 项目 ----------
        .route("/api/toys", web::get().to(showcase::toys::list))
        .route("/api/toys", web::post().to(showcase::toys::create))
        .route("/api/toys/batch-delete", web::post().to(showcase::toys::batch_delete))
.route("/api/toys/{slug}", web::get().to(showcase::toys::get))
        .route("/api/toys/{slug}", web::put().to(showcase::toys::update))
        .route("/api/toys/{slug}", web::delete().to(showcase::toys::delete))
        // ---------- 工具 ----------
        .route("/api/tools", web::get().to(showcase::tools::list))
        .route("/api/tools", web::post().to(showcase::tools::create))
        .route("/api/tools/batch-delete", web::post().to(showcase::tools::batch_delete))
.route("/api/tools/{slug}", web::get().to(showcase::tools::get))
        .route("/api/tools/{slug}", web::put().to(showcase::tools::update))
        .route("/api/tools/{slug}", web::delete().to(showcase::tools::delete))
        // ---------- 私信 ----------
        .route("/api/messages/conversations", web::get().to(messages::list_conversations))
        .route("/api/messages/unread", web::get().to(messages::unread))
        .route("/api/messages", web::post().to(messages::send))
        .route("/api/messages/{peer}/read", web::post().to(messages::mark_read))
        .route("/api/messages/{peer}", web::get().to(messages::history))
        // ---------- WebSocket（私信实时推送） ----------
        .route("/api/ws", web::get().to(messages::ws_route))
        // ---------- 图床 ----------
        .route("/api/upload", web::post().to(upload::upload_file))
        .route("/api/images", web::get().to(upload::list_images))
        .route("/api/images/{filename}", web::delete().to(upload::delete_image))
        // 静态资源
        .service(Files::new("/static", paths::static_dir()))
        // 兜底
        .default_service(web::route().to(not_found));
}

pub async fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::error(404, "接口或资源不存在"))
}
