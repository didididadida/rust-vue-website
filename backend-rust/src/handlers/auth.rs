//! 登录接口
//!
//! 逻辑描述：
//!   输入 → 输出：用户名 + 密码 → JWT 与用户信息
//!   关键机制：
//!     1. 限速检查放在 bcrypt 校验**之前**——
//!        bcrypt 是刻意慢的，若放在后面，攻击者可以靠大量请求消耗 CPU；
//!     2. 认证失败统一返回「用户名或密码错误」，不区分账号是否存在；
//!     3. 登录成功后确保个人资料记录存在，以便立刻能进个人主页。
//!   不变量：
//!     - 未通过邮箱验证的账号不能登录；
//!     - 失败与成功都会更新限速计数。
//!   边界：
//!     - 空用户名或空密码直接拒绝，不做数据库查询。
//!
//! 依据：
//!   经典问题：认证接口的暴力破解防护（brute-force protection）
//!   来源：
//!     1. OWASP Authentication Cheat Sheet
//!        https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
//!        要点：
//!          - 对失败尝试限速，且"限速应在验证密码之前"，
//!            否则攻击者仍可强制服务端反复执行昂贵的哈希；
//!          - 认证失败返回统一信息，避免账号枚举。
//!     2. 本项目原实现 `handlers/auth.rs`：
//!        原注释："限速检查放在 bcrypt 之前——bcrypt 是刻意慢的，
//!        如果放在后面，限速本身就成了可以被刷的计算资源"
//!   原型结论：
//!     1. 先限速后验密，是正确的顺序；
//!     2. 统一错误信息可降低账号枚举风险（与 services/user.rs 的判定一致）。
//!   本处变体：用户存储从 users.txt 迁到数据库，认证委托 `services::user`。
//!   沿用结论：保留"先限速、后验密"的顺序与统一错误信息。
//!   偏离处：无。

use actix_web::{web, HttpResponse};
use sea_orm::DatabaseConnection;

use crate::errors::AppError;
use crate::middleware;
use crate::models::{ApiResponse, LoginRequest, TokenResponse};
use crate::ratelimit;
use crate::services::{profile, user};

/// POST /api/login
pub async fn login(
    db: web::Data<DatabaseConnection>,
    body: web::Json<LoginRequest>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse, AppError> {
    let body = body.into_inner();
    let username = body.username.trim();

    if username.is_empty() || body.password.is_empty() {
        return Err(AppError::BadRequest("请输入用户名和密码".to_string()));
    }

    // 限速在验密之前：bcrypt 很慢，不能让它成为被刷的资源
    ratelimit::check(&req, username)?;

    match user::authenticate(db.as_ref(), username, &body.password).await? {
        Some(u) => {
            // 邮箱未验证的账号不允许登录（历史账号视为已验证）
            if !u.verified {
                return Err(AppError::Forbidden("该账号尚未完成邮箱验证".to_string()));
            }

            ratelimit::record_success(&u.username);

            let token = middleware::create_jwt(&u.username, u.is_admin)?;

            // 确保资料记录存在，便于登录后立即访问个人主页
            let p = profile::load(db.as_ref(), &u.username).await?;
            if p.nickname.is_empty() && p.avatar.is_empty() && p.bio.is_empty() {
                let _ = profile::save(db.as_ref(), &u.username, &p).await;
            }

            log::info!("用户 {} 登录成功", u.username);

            Ok(HttpResponse::Ok().json(ApiResponse::success(TokenResponse {
                token,
                username: u.username.clone(),
                is_admin: u.is_admin,
                nickname: if p.nickname.is_empty() {
                    u.display_name().to_string()
                } else {
                    p.nickname.clone()
                },
                avatar: p.avatar.clone(),
            })))
        }
        None => {
            log::warn!("登录失败：用户名或密码错误（{username}）");
            ratelimit::record_failure(&req, username);
            Err(AppError::Unauthorized("用户名或密码错误".to_string()))
        }
    }
}

// `/api/me` 在 handlers::register::me
