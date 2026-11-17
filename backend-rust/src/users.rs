//! 用户存储：`data/users.txt`（纯文本，一行一个用户）。
//!
//! 格式演进：
//!
//! ```text
//! # 旧格式（3 列）：用户名:bcrypt哈希:角色
//! admin:$2b$12$....:admin
//!
//! # 新格式（6 列）：用户名:哈希:角色:邮箱:已验证(0|1):昵称
//! 旧行仍然兼容 —— 缺少的列按默认值补齐（邮箱空、已验证=1、昵称=用户名）
//! ```
//!
//! 旧行视为已验证，是因为它们都是本站管理员手工创建的历史账号，
//! 重新要求邮箱验证会把现有账号直接锁死。
//!
//! 首次启动且文件中没有任何有效用户时，会自动创建管理员账号。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crate::errors::AppError;
use crate::paths;

#[derive(Debug, Clone)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    /// 邮箱，可能为空（历史账号）
    pub email: String,
    /// 邮箱是否已验证
    pub verified: bool,
    /// 展示用昵称，为空时回退到用户名
    pub nickname: String,
}

impl User {
    /// 对外展示的名字：优先昵称
    pub fn display_name(&self) -> &str {
        if self.nickname.trim().is_empty() {
            &self.username
        } else {
            &self.nickname
        }
    }
}

const HEADER: &str = "# 每行格式：用户名:bcrypt哈希:角色(admin|user):邮箱:已验证(0|1):昵称\n# 以 # 开头的行为注释；修改后无需重启服务\n";

/// 写操作串行化：注册、改资料都是「读-改-写」，不加锁会丢数据
fn write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 从 users.txt 读取所有有效用户
pub fn load() -> Vec<User> {
    let path = paths::users_file();
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(_) => return Vec::new(),
    };

    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(parse_line)
        .collect()
}

fn parse_line(line: &str) -> Option<User> {
    // 六列用 splitn(6) 保留昵称里的冒号
    let mut parts = line.splitn(6, ':');
    let username = parts.next()?.trim().to_string();
    let password_hash = parts.next()?.trim().to_string();
    let role = parts.next().unwrap_or("user").trim();
    let email = parts.next().unwrap_or("").trim().to_string();
    // 缺失的「已验证」列按 1 处理：旧行都是历史账号，不该被锁
    let verified = match parts.next() {
        Some(v) => matches!(v.trim(), "1" | "true" | "yes"),
        None => true,
    };
    let nickname = parts.next().unwrap_or("").trim().to_string();

    if username.is_empty() || password_hash.is_empty() {
        log::warn!("users.txt 中存在格式错误的行，已忽略：{}", line);
        return None;
    }
    Some(User {
        username,
        password_hash,
        is_admin: role.eq_ignore_ascii_case("admin")
            || role == "1"
            || role.eq_ignore_ascii_case("true"),
        email,
        verified,
        nickname,
    })
}

fn serialize_line(u: &User) -> String {
    // 昵称里可能出现冒号，替换成全角避免破坏列结构
    let nick = u.nickname.replace(':', "：");
    format!(
        "{}:{}:{}:{}:{}:{}\n",
        u.username,
        u.password_hash,
        if u.is_admin { "admin" } else { "user" },
        u.email,
        if u.verified { "1" } else { "0" },
        nick
    )
}

/// 校验用户名密码
pub fn authenticate(username: &str, password: &str) -> Result<Option<User>, AppError> {
    let Some(user) = load()
        .into_iter()
        .find(|u| u.username.eq_ignore_ascii_case(username.trim()))
    else {
        return Ok(None);
    };

    if bcrypt::verify(password, &user.password_hash)? {
        Ok(Some(user))
    } else {
        Ok(None)
    }
}

/// 按用户名查找
pub fn find(username: &str) -> Option<User> {
    load()
        .into_iter()
        .find(|u| u.username.eq_ignore_ascii_case(username.trim()))
}

/// 邮箱是否已被占用（排除指定用户名）
pub fn email_taken(email: &str, except: Option<&str>) -> bool {
    let e = email.trim().to_lowercase();
    if e.is_empty() {
        return false;
    }
    load().iter().any(|u| {
        u.email.to_lowercase() == e
            && except
                .map(|x| !u.username.eq_ignore_ascii_case(x))
                .unwrap_or(true)
    })
}

/// 确保至少存在一个管理员；返回是否新建了账号
pub fn ensure_admin() -> bool {
    let users = load();
    if users.iter().any(|u| u.is_admin) {
        return false;
    }

    let username = std::env::var("ADMIN_USERNAME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "admin".to_string());
    let (password, generated) = match std::env::var("ADMIN_PASSWORD") {
        Ok(v) if !v.trim().is_empty() => (v, false),
        _ => (random_password(), true),
    };

    match bcrypt::hash(&password, bcrypt::DEFAULT_COST) {
        Ok(hash) => {
            if let Err(e) = append_user(&username, &hash, true) {
                log::error!("创建管理员账号失败: {}", e);
                return false;
            }
            if generated {
                log::warn!("======================================================");
                log::warn!(" 已创建管理员账号，请立即登录并妥善保存：");
                log::warn!("   用户名: {}", username);
                log::warn!("   密  码: {}", password);
                log::warn!(" （密码仅本次输出，可在 data/users.txt 中自行替换哈希）");
                log::warn!("======================================================");
            } else {
                log::info!(
                    "已按 ADMIN_USERNAME/ADMIN_PASSWORD 创建管理员账号: {}",
                    username
                );
            }
            true
        }
        Err(e) => {
            log::error!("密码哈希计算失败: {}", e);
            false
        }
    }
}

/// 追加一个用户（保留已有内容与注释）
pub fn append_user(username: &str, password_hash: &str, is_admin: bool) -> Result<(), AppError> {
    append_full(&User {
        username: username.trim().to_string(),
        password_hash: password_hash.to_string(),
        is_admin,
        email: String::new(),
        verified: true,
        nickname: String::new(),
    })
}

/// 追加完整用户记录
pub fn append_full(user: &User) -> Result<(), AppError> {
    let _guard = write_lock().lock().unwrap_or_else(|e| e.into_inner());
    let path = paths::users_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let need_header = !path.exists() || fs::metadata(&path).map(|m| m.len() == 0).unwrap_or(true);

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    if need_header {
        file.write_all(HEADER.as_bytes())?;
    }
    file.write_all(serialize_line(user).as_bytes())?;
    Ok(())
}

/// 整体重写用户表（改资料、标记验证状态时用）
fn rewrite_all(users: &[User]) -> Result<(), AppError> {
    let path = paths::users_file();
    let mut out = String::from(HEADER);
    for u in users {
        out.push_str(&serialize_line(u));
    }
    fs::write(&path, out)?;
    Ok(())
}

/// 更新指定用户的若干字段；返回是否找到该用户
///
/// 用 FnMut 而不是 FnOnce：闭包在遍历中被调用，但用户名唯一，
/// 实际只会命中一次。FnMut 让编译器不要求 Copy，也不会因循环而报 moved。
pub fn update<F>(username: &str, mut f: F) -> Result<bool, AppError>
where
    F: FnMut(&mut User),
{
    let _guard = write_lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut users = load();
    let mut found = false;
    for u in users.iter_mut() {
        if u.username.eq_ignore_ascii_case(username.trim()) {
            f(u);
            found = true;
        }
    }
    if found {
        rewrite_all(&users)?;
    }
    Ok(found)
}

/// 生成随机密码（用于首次启动的兜底管理员）
pub fn random_password() -> String {
    let hex = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    hex[..16].to_string()
}
