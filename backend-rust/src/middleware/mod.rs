use actix_web::HttpRequest;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use std::sync::OnceLock;

use crate::errors::AppError;
use crate::models::Claims;

/// 进程级 JWT 密钥，由 `init_jwt_secret()` 在启动时初始化一次，
/// 保证多 worker 之间一致。
static JWT_SECRET: OnceLock<String> = OnceLock::new();

const MIN_SECRET_LEN: usize = 16;

/// 密钥文件：放在数据目录下（该目录已被 .gitignore 排除）
fn secret_file() -> std::path::PathBuf {
    std::env::var("JWT_SECRET_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| crate::paths::data_dir().join("jwt.secret"))
}

/// 生成并持久化密钥。
///
/// 为什么必须落盘：之前未设置 `JWT_SECRET` 时每次启动随机生成，
/// 结果每次重启都把所有人踢下线；一旦部署脚本里带 restart，
/// 就是「发版即全员掉登录」。落盘后密钥跨重启稳定。
///
/// 返回 (密钥, 是否为 Cargo 工作目录) —— 工作目录下生成的密钥可能随
/// `cargo clean` 或 checkout 一起消失，main 里据此给出提示。
fn load_or_create_secret() -> (String, bool) {
    let path = secret_file();
    if let Ok(s) = std::fs::read_to_string(&path) {
        let trimmed = s.trim().to_string();
        if trimmed.len() >= MIN_SECRET_LEN {
            return (trimmed, false);
        }
    }

    let generated = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut in_cwd = false;
    match path.parent() {
        Some(dir) => {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log::warn!("无法创建密钥目录 {}: {}", dir.display(), e);
                in_cwd = true;
            }
        }
        None => in_cwd = true,
    }
    match std::fs::write(&path, &generated) {
        Ok(_) => log::info!("已生成并保存 JWT 密钥到 {}", path.display()),
        Err(e) => log::warn!(
            "JWT 密钥无法写入 {}（{}），本次使用临时密钥：重启后需重新登录",
            path.display(),
            e
        ),
    }
    (generated, in_cwd)
}

pub fn init_jwt_secret() {
    let secret = match std::env::var("JWT_SECRET") {
        Ok(v) if v.trim().len() >= MIN_SECRET_LEN => v,
        Ok(v) if !v.trim().is_empty() => {
            log::warn!(
                "JWT_SECRET 过短（少于 {} 个字符），仍在用；建议改成随机长密钥",
                MIN_SECRET_LEN
            );
            v
        }
        _ => load_or_create_secret().0,
    };
    let _ = JWT_SECRET.set(secret);
}

fn secret() -> String {
    JWT_SECRET
        .get()
        .cloned()
        .unwrap_or_else(|| "wanle-dev-only-secret".to_string())
}

/// 签发 token（默认 24 小时，可用 JWT_TTL_HOURS 调整）
pub fn create_jwt(username: &str, is_admin: bool) -> Result<String, AppError> {
    let hours: i64 = std::env::var("JWT_TTL_HOURS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|h| (1..=24 * 365).contains(h))
        .unwrap_or(24);

    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(hours))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: username.to_string(),
        exp: expiration,
        is_admin,
    };

    let secret = secret();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(Into::into)
}

fn verify_token(token: &str) -> Result<Claims, AppError> {
    let secret = secret();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized("登录状态已失效，请重新登录".to_string()))
}

fn extract_token(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 已登录则返回身份，未登录返回 None（用于“草稿仅登录可见”等场景）
pub fn current_user(req: &HttpRequest) -> Option<Claims> {
    extract_token(req).and_then(|token| verify_token(&token).ok())
}

pub fn verify_user(req: HttpRequest) -> Result<Claims, AppError> {
    current_user(&req).ok_or_else(|| AppError::Unauthorized("请先登录".to_string()))
}

pub fn verify_admin(req: HttpRequest) -> Result<Claims, AppError> {
    let claims = verify_user(req)?;
    if claims.is_admin {
        Ok(claims)
    } else {
        Err(AppError::Forbidden("需要管理员权限".to_string()))
    }
}
