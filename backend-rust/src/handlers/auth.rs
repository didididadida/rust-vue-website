use actix_web::{web, HttpResponse};

use crate::errors::AppError;
use crate::middleware;
use crate::models::{ApiResponse, LoginRequest, TokenResponse};
use crate::profile;
use crate::ratelimit;
use crate::users;

/// POST /api/login —— 校验 data/users.txt 中的用户
///
/// 限速：同一用户名 15 分钟内失败 5 次即锁 15 分钟。
/// 限速检查放在 bcrypt 之前——bcrypt 是刻意慢的，如果放在后面，
/// 限速本身就成了可以被刷的计算资源（每个请求都白算一次哈希）。
pub async fn login(
    body: web::Json<LoginRequest>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse, AppError> {
    let body = body.into_inner();
    let username = body.username.trim();

    if username.is_empty() || body.password.is_empty() {
        return Err(AppError::BadRequest("请输入用户名和密码".to_string()));
    }

    ratelimit::check(&req, username)?;

    match users::authenticate(username, &body.password)? {
        Some(user) => {
            // 邮箱未验证的账号不允许登录（历史账号视为已验证）
            if !user.verified {
                return Err(AppError::Forbidden("该账号尚未完成邮箱验证".to_string()));
            }
            ratelimit::record_success(&user.username);
            let token = middleware::create_jwt(&user.username, user.is_admin)?;
            // 确保资料存在，以便登录后立刻能进个人主页
            let p = profile::ensure(&user.username);
            log::info!("用户 {} 登录成功", user.username);
            Ok(HttpResponse::Ok().json(ApiResponse::success(TokenResponse {
                token,
                username: user.username.clone(),
                is_admin: user.is_admin,
                nickname: if p.nickname.is_empty() {
                    user.display_name().to_string()
                } else {
                    p.nickname.clone()
                },
                avatar: p.avatar.clone(),
            })))
        }
        None => {
            log::warn!("登录失败：用户名或密码错误（{}）", username);
            ratelimit::record_failure(&req, username);
            Err(AppError::Unauthorized("用户名或密码错误".to_string()))
        }
    }
}

// `/api/me` 已迁移到 handlers::register::me（需要返回完整资料）
