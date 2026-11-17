//! 邮箱验证码 / 验证令牌存储：`data/verifications.json`
//!
//! 设计取舍：
//!
//! 1. **令牌只存哈希**。数据库里存明文令牌，一旦文件泄露就等于所有
//!    待验证邮箱可被接管；这里存 sha256(token)，校验时比对哈希。
//! 2. **一次性**。验证成功后立刻删除记录，重放无效。
//! 3. **有过期**。默认 15 分钟（可用 `VERIFY_TTL_MINUTES` 调整）。
//! 4. **每位邮箱同时只保留最新一条**。重新发送会覆盖旧码，避免
//!    旧码仍可用导致的时序混乱。
//! 5. **限制发送频率**。默认 60 秒内同一邮箱只能发一次
//!    （`VERIFY_RESEND_SECONDS`）。没有这层限制，邮件接口会被刷成骚扰工具。
//!
//! 依据：
//!   经典问题：一次性令牌（OTP / one-time token）+ 限流防重放
//!   本处变体：无数据库，令牌表就是一个小 JSON 文件，整表读改写；
//!             用进程级 Mutex 串行化，等价于数据库的行锁语义
//!   沿用结论：令牌单次有效、有时限、有冷却，可抵御重放与暴力枚举
//!   偏离处：单机有效，多实例部署时令牌表不共享（本项目为单机，可接受）

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::paths;

/// 一条待验证记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pending {
    /// 邮箱（小写）
    pub email: String,
    /// 申请的用户名
    pub username: String,
    /// 验证码的 sha256（十六进制）
    pub code_hash: String,
    /// bcrypt 密码哈希（已验证后才写入 users.txt）
    #[serde(default)]
    pub password_hash: String,
    /// 上次发送时间（Unix 秒）
    pub sent_at: i64,
    /// 过期时间（Unix 秒）
    pub expires_at: i64,
    /// 已输错次数，超过 5 次作废（防暴力猜码）
    #[serde(default)]
    pub attempts: u32,
}

type Table = HashMap<String, Pending>;

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn store_path() -> PathBuf {
    paths::data_dir().join("verifications.json")
}

fn read_table() -> Table {
    match fs::read_to_string(store_path()) {
        Ok(raw) if !raw.trim().is_empty() => serde_json::from_str(&raw).unwrap_or_default(),
        _ => Table::new(),
    }
}

fn write_table(t: &Table) -> Result<(), AppError> {
    let path = store_path();
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let text = serde_json::to_string_pretty(t)
        .map_err(|e| AppError::Internal(format!("验证表序列化失败: {}", e)))?;
    fs::write(&path, format!("{}\n", text))?;
    Ok(())
}

/// 配置：验证码有效期（分钟）
pub fn ttl_seconds() -> i64 {
    std::env::var("VERIFY_TTL_MINUTES")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|m| (1..=1440).contains(m))
        .unwrap_or(15)
        * 60
}

/// 配置：重发冷却（秒）
pub fn resend_seconds() -> i64 {
    std::env::var("VERIFY_RESEND_SECONDS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|s| (0..=3600).contains(s))
        .unwrap_or(60)
}

/// 同一邮箱最多尝试次数
const MAX_ATTEMPTS: u32 = 5;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn normalize(email: &str) -> String {
    email.trim().to_lowercase()
}

/// 基础邮箱格式校验（足够挡住明显垃圾，不追求 RFC 完备）
pub fn is_valid_email(email: &str) -> bool {
    let e = email.trim();
    if e.len() < 5 || e.len() > 254 || e.contains(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = e.rsplit_once('@') else {
        return false;
    };
    if local.is_empty() || local.len() > 64 {
        return false;
    }
    !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
}

/// 检查是否处于冷却期；返回剩余秒数
pub fn cooldown_remaining(email: &str) -> i64 {
    let _guard = lock().lock().unwrap_or_else(|e| e.into_inner());
    let t = read_table();
    match t.get(&normalize(email)) {
        Some(p) => (p.sent_at + resend_seconds() - now()).max(0),
        None => 0,
    }
}

/// 写入（或覆盖）一条待验证记录，返回明文验证码
///
/// 验证码为 6 位数字；用 `rand` 不引入新依赖，改用 uuid 的随机性派生。
pub fn issue(email: &str, username: &str, password_hash: &str) -> Result<String, AppError> {
    let _guard = lock().lock().unwrap_or_else(|e| e.into_inner());

    let code = random_code();
    let email = normalize(email);
    let mut t = read_table();
    let at = now();
    t.insert(
        email.clone(),
        Pending {
            email,
            username: username.trim().to_string(),
            code_hash: sha256_hex(&code),
            password_hash: password_hash.to_string(),
            sent_at: at,
            expires_at: at + ttl_seconds(),
            attempts: 0,
        },
    );
    // 顺手清掉已过期的记录，避免文件无限增长
    t.retain(|_, p| p.expires_at > at);
    write_table(&t)?;
    Ok(code)
}

/// 6 位数字验证码
fn random_code() -> String {
    // 用两个 uuid 拼 32 位十六进制，转成十进制后取末 6 位，分布足够均匀
    let a = uuid::Uuid::new_v4().simple().to_string();
    let b = uuid::Uuid::new_v4().simple().to_string();
    let hex = format!("{}{}", a, b);
    let mut acc: u128 = 0;
    for ch in hex.chars() {
        acc = acc
            .wrapping_mul(16)
            .wrapping_add(ch.to_digit(16).unwrap_or(0) as u128);
    }
    format!("{:06}", acc % 1_000_000)
}

/// 校验验证码。成功返回对应的 Pending（并已从表中移除）
pub fn verify(email: &str, code: &str) -> Result<Pending, AppError> {
    let _guard = lock().lock().unwrap_or_else(|e| e.into_inner());
    let email_key = normalize(email);
    let mut t = read_table();

    let Some(p) = t.get_mut(&email_key) else {
        return Err(AppError::BadRequest("验证码已失效，请重新获取".to_string()));
    };
    if p.expires_at < now() {
        t.remove(&email_key);
        let _ = write_table(&t);
        return Err(AppError::BadRequest("验证码已过期，请重新获取".to_string()));
    }
    if p.attempts >= MAX_ATTEMPTS {
        t.remove(&email_key);
        let _ = write_table(&t);
        return Err(AppError::BadRequest(
            "尝试次数过多，请重新获取验证码".to_string(),
        ));
    }

    if p.code_hash != sha256_hex(code.trim()) {
        p.attempts += 1;
        let remaining = MAX_ATTEMPTS.saturating_sub(p.attempts);
        let _ = write_table(&t);
        return Err(AppError::BadRequest(format!(
            "验证码不正确，还可尝试 {} 次",
            remaining
        )));
    }

    // 匹配成功：取出并删除，保证一次性
    let taken = t.remove(&email_key).expect("just checked");
    let _ = write_table(&t);
    Ok(taken)
}

/// 撤回某邮箱的待验证记录（例如验证成功后清理）
pub fn discard(email: &str) -> Result<(), AppError> {
    let _guard = lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut t = read_table();
    if t.remove(&normalize(email)).is_some() {
        write_table(&t)?;
    }
    Ok(())
}

/// 待验证数量（健康检查用）
pub fn pending_count() -> usize {
    let _guard = lock().lock().unwrap_or_else(|e| e.into_inner());
    read_table().len()
}
