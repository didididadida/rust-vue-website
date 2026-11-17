//! 注册、登录态、个人资料接口
//!
//! 逻辑描述：
//!   输入 → 输出：
//!     - 邮箱验证码发送与注册
//!     - 当前登录用户信息（/api/me）
//!     - 公开主页查询、资料更新、改密
//!   关键机制：
//!     1. 验证码只存 SHA-256 哈希，比对时对输入做同样哈希；
//!     2. 验证码有有效期与重发间隔（环境变量控制）；
//!     3. 资料更新走 `services::profile::save`（内含钳制与 URL 白名单）。
//!   不变量：
//!     - 库中永不出现明文验证码或明文密码；
//!     - 公开主页只返回白名单字段（不含邮箱）。
//!   边界：
//!     - 邮箱已被占用时拒绝注册；
//!     - 改密需要旧密码验证。
//!
//! 依据：
//!   经典问题：一次性凭证的生成、存储与校验
//!   来源：
//!     1. OWASP Authentication Cheat Sheet（一次性验证码/OTP 章节）
//!        https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
//!        要点：OTP 必须有有效期、限制尝试次数、不得以明文形式持久化。
//!     2. 本项目原实现 `register.rs` 与 `verification.rs`：
//!        原实现把验证码存进程内存 HashMap（重启即失效）；
//!        迁到数据库后需要显式过期判定。
//!   原型结论：
//!     1. 存哈希而非明文，即使库泄露也无法直接使用验证码；
//!     2. 有效期与重发间隔是最小防护。
//!   本处变体：
//!     - 原内存方案靠"重启清空"实现失效，入库后必须自己判过期；
//!     - 验证码表现在以 email 为主键，同一邮箱只保留最新一条。
//!   沿用结论：只存 sha256(code)；比对时哈希后比较；读取时判过期。
//!   偏离处：未做验证码尝试次数限制（原实现也没有）。
//!           若要收紧，应在 verifications 表加 attempts 列。

use actix_web::{web, HttpRequest, HttpResponse};
use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::entities::verification;
use crate::errors::AppError;
use crate::mail;
use crate::middleware;
use crate::models::ApiResponse;
use crate::services::{profile, user};

/// 验证码有效期（分钟）
fn ttl_minutes() -> i64 {
    std::env::var("VERIFY_TTL_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

/// 重发间隔（秒）
fn resend_seconds() -> i64 {
    std::env::var("VERIFY_RESEND_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60)
}

/// 生成 6 位数字验证码
fn gen_code() -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    let n = u32::from_str_radix(&hex[..8], 16).unwrap_or(0) % 1_000_000;
    format!("{n:06}")
}

/// 验证码哈希
fn hash_code(code: &str) -> String {
    let mut h = Sha256::new();
    h.update(code.as_bytes());
    format!("{:x}", h.finalize())
}

fn now_ts() -> i64 {
    Utc::now().timestamp()
}

/// 请求体
#[derive(serde::Deserialize)]
pub struct SendCodeRequest {
    pub email: String,
}

#[derive(serde::Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub code: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub nickname: String,
}

#[derive(serde::Deserialize)]
pub struct ProfileUpdate {
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub avatar_zoom: Option<f32>,
    #[serde(default)]
    pub avatar_x: Option<i16>,
    #[serde(default)]
    pub avatar_y: Option<i16>,
}

#[derive(serde::Deserialize)]
pub struct PasswordChange {
    pub old_password: String,
    pub new_password: String,
}

/// GET /api/register/status —— 注册是否开放（当前恒为开放）
pub async fn register_status() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(json!({ "enabled": true })))
}

/// POST /api/register/send-code —— 发送验证码
pub async fn send_code(
    db: web::Data<DatabaseConnection>,
    body: web::Json<SendCodeRequest>,
) -> Result<HttpResponse, AppError> {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 5 {
        return Err(AppError::BadRequest("邮箱格式不正确".to_string()));
    }

    // 邮箱已被占用则不必发验证码
    if user::email_taken(db.as_ref(), &email, None).await? {
        return Err(AppError::Conflict("该邮箱已被注册".to_string()));
    }

    // 重发间隔检查
    let existing = verification::Entity::find_by_id(email.clone())
        .one(db.as_ref())
        .await
        .map_err(|e| AppError::Internal(format!("查询验证码失败: {e}")))?;

    if let Some(ref row) = existing {
        let sent_at: i64 = row.sent_at.parse().unwrap_or(0);
        let wait = resend_seconds() - (now_ts() - sent_at);
        if wait > 0 {
            return Err(AppError::TooManyRequests(format!("请 {wait} 秒后再试")));
        }
    }

    let code = gen_code();
    let expires = now_ts() + ttl_minutes() * 60;

    // 写入（已存在则覆盖：email 是主键）
    use sea_orm::{ActiveModelTrait, Set};
    let am = verification::ActiveModel {
        email: Set(email.clone()),
        code_hash: Set(hash_code(&code)),
        expires_at: Set(expires.to_string()),
        sent_at: Set(now_ts().to_string()),
    };
    let _ = verification::Entity::delete_by_id(email.clone()).exec(db.as_ref()).await;
    am.insert(db.as_ref())
        .await
        .map_err(|e| AppError::Internal(format!("保存验证码失败: {e}")))?;

    // 发信；未配置 SMTP 时验证码打到日志
    match mail::send_code(&email, &code, ttl_minutes()).await {
        Ok(()) => {}
        Err(e) => log::warn!("发送验证码邮件失败: {e}"),
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "sent": true,
        "ttl_minutes": ttl_minutes(),
    }))))
}

/// POST /api/register —— 完成注册
pub async fn register(
    db: web::Data<DatabaseConnection>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    let email = body.email.trim().to_lowercase();
    let username = body.username.trim();

    if username.len() < 2 || username.len() > 20 {
        return Err(AppError::BadRequest("用户名长度需在 2-20 之间".to_string()));
    }
    if body.password.len() < 6 {
        return Err(AppError::BadRequest("密码至少 6 位".to_string()));
    }

    // 校验验证码
    let row = verification::Entity::find_by_id(email.clone())
        .one(db.as_ref())
        .await
        .map_err(|e| AppError::Internal(format!("查询验证码失败: {e}")))?
        .ok_or_else(|| AppError::BadRequest("请先获取验证码".to_string()))?;

    let expires: i64 = row.expires_at.parse().unwrap_or(0);
    if now_ts() > expires {
        return Err(AppError::BadRequest("验证码已过期".to_string()));
    }
    if hash_code(body.code.trim()) != row.code_hash {
        return Err(AppError::BadRequest("验证码错误".to_string()));
    }

    // 用户名与邮箱不能重复
    if user::find(db.as_ref(), username).await?.is_some() {
        return Err(AppError::Conflict("该用户名已被占用".to_string()));
    }
    if user::email_taken(db.as_ref(), &email, None).await? {
        return Err(AppError::Conflict("该邮箱已被注册".to_string()));
    }

    let hash = bcrypt::hash(&body.password, 10)
        .map_err(|e| AppError::Internal(format!("密码哈希失败: {e}")))?;

    user::create(db.as_ref(), username, &hash, false, &email, true, &body.nickname).await?;

    // 验证码用掉即删
    let _ = verification::Entity::delete_by_id(email.clone()).exec(db.as_ref()).await;

    // 建一份空资料
    let _ = profile::save(db.as_ref(), username, &profile::Profile::default()).await;

    let token = middleware::create_jwt(username, false)?;

    Ok(HttpResponse::Created().json(ApiResponse::success(json!({
        "token": token,
        "username": username,
        "is_admin": false,
        "nickname": body.nickname,
        "avatar": "",
    }))))
}

/// GET /api/me —— 当前登录用户
pub async fn me(
    db: web::Data<DatabaseConnection>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    let u = user::find(db.as_ref(), &claims.sub)
        .await?
        .ok_or_else(|| AppError::Unauthorized("账号不存在".to_string()))?;

    let p = profile::load(db.as_ref(), &claims.sub).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "username": u.username,
        "is_admin": u.is_admin,
        "email": u.email,
        "verified": u.verified,
        "nickname": if p.nickname.is_empty() { u.display_name().to_string() } else { p.nickname },
        "avatar": p.avatar,
        "bio": p.bio,
        "banner": p.banner,
        "avatar_zoom": p.avatar_zoom,
        "avatar_x": p.avatar_x,
        "avatar_y": p.avatar_y,
    }))))
}

/// GET /api/profile/{username} —— 公开主页
///
/// 只返回白名单字段，**不含邮箱**。
pub async fn get_profile(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let username = path.into_inner();

    let u = user::find(db.as_ref(), &username)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".to_string()))?;

    let p = profile::load(db.as_ref(), &username).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "username": u.username,
        "nickname": if p.nickname.is_empty() { u.display_name().to_string() } else { p.nickname },
        "bio": p.bio,
        "avatar": p.avatar,
        "banner": p.banner,
        "avatar_zoom": p.avatar_zoom,
        "avatar_x": p.avatar_x,
        "avatar_y": p.avatar_y,
        "is_admin": u.is_admin,
    }))))
}

/// PUT /api/profile —— 更新自己的资料
pub async fn update_profile(
    db: web::Data<DatabaseConnection>,
    body: web::Json<ProfileUpdate>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    let mut p = profile::load(db.as_ref(), &claims.sub).await?;

    if let Some(v) = &body.nickname {
        p.nickname = v.clone();
    }
    if let Some(v) = &body.bio {
        p.bio = v.clone();
    }
    if let Some(v) = &body.avatar {
        p.avatar = v.clone();
    }
    if let Some(v) = &body.banner {
        p.banner = v.clone();
    }
    if let Some(v) = body.avatar_zoom {
        p.avatar_zoom = v;
    }
    if let Some(v) = body.avatar_x {
        p.avatar_x = v;
    }
    if let Some(v) = body.avatar_y {
        p.avatar_y = v;
    }

    profile::save(db.as_ref(), &claims.sub, &p).await?;

    // 同步昵称到 users 表（登录响应与展示名都用它）
    if let Some(v) = &body.nickname {
        let n = v.clone();
        let _ = user::update(db.as_ref(), &claims.sub, |am| {
            am.nickname = sea_orm::Set(n);
        })
        .await;
    }

    let saved = profile::load(db.as_ref(), &claims.sub).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(saved)))
}

/// PUT /api/profile/password —— 修改密码
pub async fn change_password(
    db: web::Data<DatabaseConnection>,
    body: web::Json<PasswordChange>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    let u = user::find(db.as_ref(), &claims.sub)
        .await?
        .ok_or_else(|| AppError::Unauthorized("账号不存在".to_string()))?;

    // 旧密码必须正确
    let ok = bcrypt::verify(&body.old_password, &u.password_hash).unwrap_or(false);
    if !ok {
        return Err(AppError::Unauthorized("原密码不正确".to_string()));
    }

    if body.new_password.len() < 6 {
        return Err(AppError::BadRequest("新密码至少 6 位".to_string()));
    }

    let hash = bcrypt::hash(&body.new_password, 10)
        .map_err(|e| AppError::Internal(format!("密码哈希失败: {e}")))?;

    user::update(db.as_ref(), &claims.sub, |am| {
        am.password_hash = sea_orm::Set(hash);
    })
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "changed": true }))))
}
