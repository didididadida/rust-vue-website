//! 应用入口
//!
//! 逻辑描述：
//!   输入 → 输出：环境变量 → 启动 HTTP 服务
//!   关键机制：
//!     1. 启动顺序：目录 → JWT 密钥 → 数据库 → 管理员账号 → 迁移 → 服务；
//!     2. 数据库连接与 WebSocket Hub 用 `web::Data` 注入，跨 worker 共享；
//!     3. 迁移只在 `AUTO_MIGRATE=1` 时自动执行，默认手动触发，
//!        避免每次重启都跑一遍（虽然幂等，但会有无用日志）。
//!   不变量：
//!     - 数据库初始化失败则不启动（宁可不服务，也不要半残状态）；
//!     - 至少存在一个管理员账号。
//!   边界：
//!     - 监听地址优先级：BIND_ADDR > PORT（云平台）> 127.0.0.1:8000；
//!     - 云平台必须绑 0.0.0.0，否则平台探测不到端口。
//!
//! 依据：
//!   经典问题：应用的启动顺序与失败快速返回（fail-fast startup）
//!   来源：
//!     1. actix-web 官方「Shared mutable state」
//!        https://actix.rs/docs/application/#shared-mutable-state
//!        要点：HttpServer::new 的闭包每个 worker 执行一次，
//!              闭包内创建的资源不会被共享，必须用 web::Data 注入。
//!     2. Render 的端口绑定要求
//!        https://render.com/tutorials/when-deploys-go-wrong/boot-and-port-binding
//!        要点：必须监听 0.0.0.0:$PORT，绑 127.0.0.1 会导致「部署成功但 502」。
//!   原型结论：
//!     1. 数据库连接必须在 HttpServer 外创建；
//!     2. 云平台只给 PORT 数字，需要拼成 0.0.0.0:PORT。
//!   本处变体：原实现没有数据库；现在多一个必须初始化的依赖。
//!   沿用结论：保留 actix 的 Data 注入方式；保留 PORT 兼容逻辑。
//!   偏离处：迁移默认不自动跑。原实现没有迁移概念；
//!           首次部署需要显式执行 `AUTO_MIGRATE=1` 或调用迁移接口。

mod db;
mod entities;
mod errors;
mod handlers;
mod images;
mod mail;
mod middleware;
mod migrate;
mod migrate_forum;
mod migrate_showcase;
mod models;
mod paths;
mod ratelimit;
mod services;
mod transcode;
mod ws;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use std::env;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // ---------- 1. 目录 ----------
    if let Err(e) = paths::ensure_dirs() {
        log::error!("初始化数据目录失败: {e}");
        return Err(e);
    }

    // ---------- 2. JWT 密钥 ----------
    middleware::init_jwt_secret();

    // ---------- 3. 数据库 ----------
    let db = match db::init().await {
        Ok(db) => db,
        Err(e) => {
            log::error!("初始化数据库失败: {e}");
            return Err(std::io::Error::other(e.to_string()));
        }
    };

    // ---------- 4. 管理员账号 ----------
    if let Err(e) = services::user::ensure_admin(&db).await {
        log::error!("初始化管理员失败: {e}");
        return Err(std::io::Error::other(e.to_string()));
    }

    // ---------- 5. 可选：自动迁移 ----------
    if env::var("AUTO_MIGRATE").ok().as_deref() == Some("1") {
        log::info!("AUTO_MIGRATE=1，开始从文件迁移到数据库…");
        match migrate::run(&db, false).await {
            Ok(report) => {
                log::info!("迁移完成:\n{}", report.summary());
                if report.has_failures() {
                    log::warn!("存在迁移失败项，请检查日志");
                }
            }
            Err(e) => log::error!("迁移失败: {e}"),
        }
    }

    if let Ok(c) = migrate::counts(&db).await {
        log::info!("当前数据: {c}");
    }

    // ---------- 6. 监听地址 ----------
    // 优先级：BIND_ADDR > PORT（云平台）> 默认
    let bind_addr = match env::var("BIND_ADDR") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => match env::var("PORT") {
            Ok(p) if !p.trim().is_empty() => format!("0.0.0.0:{}", p.trim()),
            _ => "127.0.0.1:8000".to_string(),
        },
    };

    log::info!("wanle 后端启动: http://{bind_addr}");
    log::info!("文章目录: {}", paths::posts_dir().display());
    log::info!("静态目录: {}", paths::static_dir().display());

    match mail::config() {
        Some(_) => log::info!("邮件服务: 已配置 SMTP"),
        None => log::warn!("邮件服务: 未配置 SMTP（MAIL_MODE=log，验证码打印到日志）"),
    }

    if transcode::ffmpeg_available() {
        log::info!("视频转码: 已启用");
    } else {
        log::warn!("视频转码: 未启用（未找到 ffmpeg）");
    }

    images::ensure_variants();

    // ---------- 7. CORS ----------
    let cors_origins = env::var("CORS_ORIGINS").unwrap_or_default();
    let allow_all = cors_origins.trim() == "*";
    let allowed: Vec<String> = cors_origins
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "*")
        .collect();

    if allow_all {
        log::warn!("CORS: 放行全部来源（*）");
    } else if !allowed.is_empty() {
        log::info!("CORS: 允许来源 {allowed:?}");
    }

    // ---------- 8. 共享状态 ----------
    // 必须在 HttpServer 之前创建：闭包每个 worker 执行一次，闭包内建的不共享
    let db_data = web::Data::new(db);
    let hub = web::Data::new(Arc::new(ws::Hub::new()));

    HttpServer::new(move || {
        let cors = if allow_all {
            Cors::default()
                .allow_any_origin()
                .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
                .allowed_headers(vec![
                    actix_web::http::header::AUTHORIZATION,
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::ACCEPT,
                ])
                .max_age(3600)
        } else {
            let mut c = Cors::default()
                .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
                .allowed_headers(vec![
                    actix_web::http::header::AUTHORIZATION,
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::ACCEPT,
                ])
                .max_age(3600);
            for origin in &allowed {
                c = c.allowed_origin(origin);
            }
            c
        };

        App::new()
            .wrap(cors)
            .app_data(db_data.clone())
            .app_data(hub.clone())
            .configure(handlers::config)
    })
    .bind(&bind_addr)?
    .run()
    .await
}

