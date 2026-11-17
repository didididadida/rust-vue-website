mod comments;
mod content;
mod db;
mod errors;
mod handlers;
mod images;
mod mail;
mod middleware;
mod models;
mod paths;
mod profile;
mod ratelimit;
mod transcode;
mod users;
mod verification;

use actix_cors::Cors;
use actix_web::{App, HttpServer};
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    if let Err(e) = paths::ensure_dirs() {
        log::error!("初始化数据目录失败: {}", e);
        return Err(e);
    }
    middleware::init_jwt_secret();
    users::ensure_admin();

    // 图床元数据库（SQLite）：建表，幂等
    if let Err(e) = db::init() {
        log::error!("初始化图床数据库失败: {}", e);
        return Err(std::io::Error::other(e.to_string()));
    }

    // 监听地址解析顺序：
    //   1. BIND_ADDR —— 完整地址，本地开发/自建服务器用
    //   2. PORT —— 云平台（Render / Railway / Fly 等）注入的端口，必须绑 0.0.0.0
    //   3. 默认 127.0.0.1:8000
    //
    // 为什么云平台要单独处理：它们只给一个 PORT 数字，且要求进程监听 0.0.0.0
    // 才能被平台的反向代理访问到；绑 127.0.0.1 会导致「部署成功但访问 502」。
    let bind_addr = match env::var("BIND_ADDR") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => match env::var("PORT") {
            Ok(p) if !p.trim().is_empty() => format!("0.0.0.0:{}", p.trim()),
            _ => "127.0.0.1:8000".to_string(),
        },
    };

    log::info!("wanle 后端启动: http://{}", bind_addr);
    log::info!("内容根目录: {}", paths::root().display());
    log::info!("文章目录: {}", paths::posts_dir().display());
    log::info!("评论目录: {}", paths::comments_dir().display());
    log::info!("图床目录: {}", paths::uploads_dir().display());
    log::info!("个人资料: {}", paths::data_dir().join("profiles").display());
    log::info!(
        "注册验证: {}",
        paths::data_dir().join("verifications.json").display()
    );

    match mail::config() {
        Some(_) => log::info!("邮件服务: 已配置 SMTP"),
        None => log::warn!("邮件服务: 未配置 SMTP（MAIL_MODE=log，验证码将打印到日志）"),
    }

    // 启动时探测 ffmpeg，决定视频转码队列是否可用（不可用则跳过后台转码）
    if transcode::ffmpeg_available() {
        log::info!("视频转码: 已启用（队列状态 {:?}）", transcode::status());
    } else {
        log::warn!("视频转码: 未启用（未找到 ffmpeg，上传的视频将直接原样播放）");
    }

    // 图片：把历史上传的静态图补做一遍无损重编码（幂等，已经压过的不会重复写盘）
    images::ensure_variants();

    // ---------- CORS ----------
    //
    // 默认不开启（同源部署不需要）。设置 CORS_ORIGINS 后才放行指定的来源，
    // 多个来源用逗号分隔；值为 `*` 时放行全部（仅建议演示环境使用）。
    //
    // 为什么默认关闭：跨域放行会让任意站点都能读取本站 API，
    // 只有"前端独立部署到别的域名"（如 GitHub Pages）时才需要打开。
    let cors_origins = env::var("CORS_ORIGINS").unwrap_or_default();
    let allow_all = cors_origins.trim() == "*";
    let allowed: Vec<String> = cors_origins
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "*")
        .collect();

    if allow_all {
        log::warn!("CORS: 放行全部来源（*），仅建议演示环境使用");
    } else if !allowed.is_empty() {
        log::info!("CORS: 允许来源 {:?}", allowed);
    }

    HttpServer::new(move || {
        // 统一构建一个 Cors 中间件：未配置时不放行任何来源，
        // 这样 App 的类型在编译期是固定的，避免分支间泛型不匹配。
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

        App::new().wrap(cors).configure(handlers::config)
    })
    .bind(&bind_addr)?
    .run()
    .await
}
