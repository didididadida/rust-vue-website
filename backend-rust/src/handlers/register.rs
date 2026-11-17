//! 注册、个人主页相关接口。

use actix_web::{web, HttpRequest, HttpResponse};

use crate::comments;
use crate::content::{self, ArticleRecord};
use crate::errors::AppError;
use crate::mail;
use crate::middleware;
use crate::models::*;
use crate::paths;
use crate::profile::{self, Profile};
use crate::users;
use crate::verification;

/// 注册用户的站内头衔
pub const USER_TITLE: &str = "船长";

const MIN_PASSWORD_LEN: usize = 6;
const MAX_PASSWORD_LEN: usize = 64;
const MIN_USERNAME_LEN: usize = 2;
const MAX_USERNAME_LEN: usize = 20;

/// 用户名规则：字母数字下划线连字符，中文也允许
fn validate_username(name: &str) -> Result<(), AppError> {
    let n = name.trim();
    let count = n.chars().count();
    if count < MIN_USERNAME_LEN {
        return Err(AppError::BadRequest(format!(
            "用户名至少 {} 个字符",
            MIN_USERNAME_LEN
        )));
    }
    if count > MAX_USERNAME_LEN {
        return Err(AppError::BadRequest(format!(
            "用户名最多 {} 个字符",
            MAX_USERNAME_LEN
        )));
    }
    // 必须是合法 slug（不能含路径分隔符等），否则会污染文件路径
    if !content::is_safe_slug(n) {
        return Err(AppError::BadRequest(
            "用户名只能包含字母、数字、下划线、连字符或中文".to_string(),
        ));
    }
    Ok(())
}

/// POST /api/register/send-code —— 发送邮箱验证码
pub async fn send_code(body: web::Json<SendCodeRequest>) -> Result<HttpResponse, AppError> {
    let body = body.into_inner();
    let email = body.email.trim().to_lowercase();
    let username = body.username.trim().to_string();
    let password = body.password;

    if !verification::is_valid_email(&email) {
        return Err(AppError::BadRequest("邮箱格式不正确".to_string()));
    }
    validate_username(&username)?;
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "密码至少 {} 个字符",
            MIN_PASSWORD_LEN
        )));
    }
    if password.chars().count() > MAX_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "密码最多 {} 个字符",
            MAX_PASSWORD_LEN
        )));
    }

    // 账号查重：用户名与邮箱都必须是新的
    if users::find(&username).is_some() {
        return Err(AppError::Conflict("这个用户名已经被注册了".to_string()));
    }
    if users::email_taken(&email, None) {
        return Err(AppError::Conflict("这个邮箱已经被注册了".to_string()));
    }

    // 冷却期内不允许重发，防止刷邮件
    let remain = verification::cooldown_remaining(&email);
    if remain > 0 {
        return Err(AppError::BadRequest(format!(
            "操作过于频繁，请 {} 秒后再试",
            remain
        )));
    }

    // 先算哈希再发信：避免发给一个连密码都存不下的请求
    let hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("密码处理失败: {}", e)))?;

    let code = verification::issue(&email, &username, &hash)?;
    let ttl_min = verification::ttl_seconds() / 60;

    // 发信失败则撤回记录，让用户可以立刻重试
    if let Err(e) = mail::send_code(&email, &code, ttl_min).await {
        let _ = verification::discard(&email);
        return Err(e);
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "sent": true,
            "expires_in": ttl_min * 60,
            "resend_after": verification::resend_seconds(),
            "email": email,
        }))),
    )
}

/// POST /api/register —— 用验证码完成注册
pub async fn register(body: web::Json<RegisterRequest>) -> Result<HttpResponse, AppError> {
    let body = body.into_inner();
    let email = body.email.trim().to_lowercase();

    if body.code.trim().is_empty() {
        return Err(AppError::BadRequest("请输入验证码".to_string()));
    }

    // 校验并取出该邮箱的待验证记录（成功后即被删除，不可重放）
    let pending = verification::verify(&email, &body.code)?;

    // 二次查重：从发码到提交之间，用户名可能已被别人抢占
    if users::find(&pending.username).is_some() {
        return Err(AppError::Conflict("这个用户名已经被注册了".to_string()));
    }
    if users::email_taken(&email, None) {
        return Err(AppError::Conflict("这个邮箱已经被注册了".to_string()));
    }

    let user = users::User {
        username: pending.username.clone(),
        password_hash: pending.password_hash.clone(),
        is_admin: false,
        email: email.clone(),
        verified: true,
        nickname: String::new(),
    };

    if let Err(e) = users::append_full(&user) {
        log::error!("写入新用户失败: {}", e);
        return Err(e);
    }
    // 建资料并记录注册时间
    profile::ensure(&pending.username);

    let token = middleware::create_jwt(&user.username, false)?;
    log::info!("新用户注册成功: {} ({})", user.username, email);

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "token": token,
            "username": user.username,
            "is_admin": false,
            "title": USER_TITLE,
        }))),
    )
}

/// GET /api/me —— 当前登录用户信息（含资料）
pub async fn me(req: HttpRequest) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let u =
        users::find(&claims.sub).ok_or_else(|| AppError::Unauthorized("账号不存在".to_string()))?;
    let p = profile::load(&u.username);

    Ok(HttpResponse::Ok().json(ApiResponse::success(MeDetail {
        username: u.username.clone(),
        nickname: if p.nickname.is_empty() {
            u.display_name().to_string()
        } else {
            p.nickname.clone()
        },
        bio: p.bio.clone(),
        avatar: p.avatar.clone(),
        banner: p.banner.clone(),
        avatar_zoom: p.avatar_zoom,
        avatar_x: p.avatar_x,
        avatar_y: p.avatar_y,
        email: u.email.clone(),
        is_admin: u.is_admin,
    })))
}

/// GET /api/profile/{username} —— 公开的个人主页
pub async fn get_profile(path: web::Path<String>) -> Result<HttpResponse, AppError> {
    let username = path.into_inner();
    let user =
        users::find(&username).ok_or_else(|| AppError::NotFound("用户不存在".to_string()))?;
    let p = profile::ensure(&user.username);

    // 统计该用户发表过的评论，并收集最近 10 条
    let mut all: Vec<(String, comments::Comment)> = Vec::new();
    if let Ok(list) = comments::list_all() {
        for (slug, c) in list {
            if c.author.eq_ignore_ascii_case(&user.username)
                || c.author.eq_ignore_ascii_case(user.display_name())
            {
                all.push((slug, c));
            }
        }
    }
    let comment_count = all.len();

    // list_all 已按时间倒序，取前 10 条并补上文章标题
    let recent: Vec<ProfileComment> = all
        .into_iter()
        .take(10)
        .map(|(slug, c)| {
            let title = content::find::<ArticleRecord>(&paths::posts_dir(), &slug)
                .ok()
                .flatten()
                .map(|d| d.data.title)
                .unwrap_or_else(|| slug.clone());
            ProfileComment {
                slug,
                article_title: title,
                content: c.content,
                created_at: c.created_at,
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(ProfileView {
        username: user.username.clone(),
        nickname: if p.nickname.is_empty() {
            user.display_name().to_string()
        } else {
            p.nickname.clone()
        },
        bio: p.bio.clone(),
        avatar: p.avatar.clone(),
        banner: p.banner.clone(),
        avatar_zoom: p.avatar_zoom,
        avatar_x: p.avatar_x,
        avatar_y: p.avatar_y,
        is_admin: user.is_admin,
        joined_at: p.joined_at.clone(),
        title: USER_TITLE.to_string(),
        comment_count,
        recent_comments: recent,
    })))
}

/// PUT /api/profile —— 修改自己的资料
pub async fn update_profile(
    body: web::Json<ProfileUpdate>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let user =
        users::find(&claims.sub).ok_or_else(|| AppError::Unauthorized("账号不存在".to_string()))?;

    let mut p = profile::ensure(&user.username);
    let body = body.into_inner();

    if let Some(email) = body.email.as_deref() {
        let e = email.trim().to_lowercase();
        // 换邮箱需要重新验证：这里只允许改成空，或与当前一致
        if !e.is_empty() && e != user.email.to_lowercase() {
            return Err(AppError::BadRequest(
                "暂不支持直接更换邮箱，请联系管理员".to_string(),
            ));
        }
    }

    // 头图与头像共用同一套 URL 校验：只放行站内路径与 http(s)，
    // 防止把 javascript: 之类的伪协议写进背景图（会变成 XSS 面）
    if let Some(banner) = body.banner.as_deref() {
        if !banner.trim().is_empty() && profile::safe_image_url(banner).is_none() {
            return Err(AppError::BadRequest(
                "头图地址必须以 / 或 http(s):// 开头".to_string(),
            ));
        }
    }

    let (nick, bio, avatar, banner) = profile::clean(
        body.nickname.as_deref().unwrap_or(&p.nickname),
        body.bio.as_deref().unwrap_or(&p.bio),
        body.avatar.as_deref().unwrap_or(&p.avatar),
        body.banner.as_deref().unwrap_or(&p.banner),
    );
    p.nickname = nick;
    p.bio = bio;
    p.avatar = avatar;
    p.banner = banner;
    // 头像裁剪参数：未传 = 保持不变；传了就归一化后写入
    if let Some(zoom) = body.avatar_zoom {
        p.avatar_zoom = profile::clamp_zoom(zoom);
    }
    if let Some(x) = body.avatar_x {
        p.avatar_x = profile::clamp_offset(x);
    }
    if let Some(y) = body.avatar_y {
        p.avatar_y = profile::clamp_offset(y);
    }
    if p.avatar_zoom < profile::AVATAR_ZOOM_MIN {
        p.avatar_zoom = profile::AVATAR_ZOOM_MIN;
    }
    profile::save(&user.username, &p)?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(profile_view_of(&user, &p))))
}

fn profile_view_of(user: &users::User, p: &Profile) -> MeDetail {
    MeDetail {
        username: user.username.clone(),
        nickname: if p.nickname.is_empty() {
            user.display_name().to_string()
        } else {
            p.nickname.clone()
        },
        bio: p.bio.clone(),
        avatar: p.avatar.clone(),
        avatar_zoom: p.avatar_zoom,
        avatar_x: p.avatar_x,
        avatar_y: p.avatar_y,
        banner: p.banner.clone(),
        email: user.email.clone(),
        is_admin: user.is_admin,
    }
}

/// PUT /api/password —— 修改自己的密码
pub async fn change_password(
    body: web::Json<PasswordChange>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let user =
        users::find(&claims.sub).ok_or_else(|| AppError::Unauthorized("账号不存在".to_string()))?;

    if !bcrypt::verify(&body.old_password, &user.password_hash)
        .map_err(|e| AppError::Internal(format!("密码校验失败: {}", e)))?
    {
        return Err(AppError::BadRequest("原密码不正确".to_string()));
    }

    let len = body.new_password.chars().count();
    if len < MIN_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "新密码至少 {} 个字符",
            MIN_PASSWORD_LEN
        )));
    }
    if len > MAX_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "新密码最多 {} 个字符",
            MAX_PASSWORD_LEN
        )));
    }

    let hash = bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("密码处理失败: {}", e)))?;
    // FnMut 可能被多次调用，闭包里不能消费 hash，用 clone 传递给赋值
    users::update(&user.username, |u| u.password_hash = hash.clone())?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({ "updated": true }))))
}

/// GET /api/register/status —— 查询邮件功能是否可用
pub async fn register_status() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "enabled": true,
        "title": USER_TITLE,
        "pending": verification::pending_count(),
    })))
}
