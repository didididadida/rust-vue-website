//! 用户服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：`users` 表的读写 —— 认证、查找、注册、改密、首次建管理员
//!   关键机制：
//!     1. 密码用 bcrypt 校验（`bcrypt::verify`），注册时 `bcrypt::hash`；
//!     2. 首次启动若没有任何管理员，按 `.env` 的 ADMIN_USERNAME/ADMIN_PASSWORD 创建；
//!     3. 邮箱唯一性靠 `email_taken` 查询保证（应用层判定，非数据库唯一索引）。
//!   不变量：
//!     - 库中永不出现明文密码；
//!     - 用户名是主键，天然唯一。
//!   边界：
//!     - 空邮箱不参与唯一性判定（历史账号可以没有邮箱）；
//!     - 认证失败统一返回 `Ok(None)`，不区分"用户不存在"与"密码错误"，
//!       避免被用来探测账号是否存在。
//!
//! 依据：
//!   经典问题：口令认证与用户名枚举防护
//!   来源：
//!     1. OWASP Authentication Cheat Sheet
//!        https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
//!        要点：认证失败应返回统一的错误，不泄露账号是否存在；
//!              密码必须用自适应哈希（bcrypt/scrypt/Argon2）。
//!     2. OWASP Password Storage Cheat Sheet
//!        https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
//!        要点：bcrypt 自带 salt，cost 建议 ≥ 10。
//!     3. 本项目原实现 `users.rs`（users.txt 版）
//!   原型结论：
//!     1. bcrypt 是符合 OWASP 推荐的存储方式，自带 salt 无需另存；
//!     2. 认证失败不区分原因，可降低账号枚举风险。
//!   本处变体：从 users.txt（一行一个，冒号分隔）迁到表；
//!             哈希串原样搬运，不重新计算（否则用户要重置密码）。
//!   沿用结论：保留 bcrypt 哈希与统一的认证失败返回。
//!   偏离处：`email_taken` 仍是应用层查询而非唯一索引——
//!           因为历史数据里可能有重复或空邮箱，加唯一索引会导致迁移失败。
//!           若要收紧，应在数据清洗后单独发一次迁移。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};

use crate::entities::{user, user::Entity as UserEntity};
use crate::errors::AppError;

/// 对外返回的用户结构
#[derive(Debug, Clone, serde::Serialize)]
pub struct User {
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub is_admin: bool,
    pub email: String,
    pub verified: bool,
    pub nickname: String,
}

impl User {
    /// 展示名：昵称为空时回退用户名
    pub fn display_name(&self) -> &str {
        if self.nickname.trim().is_empty() {
            &self.username
        } else {
            &self.nickname
        }
    }
}

impl From<user::Model> for User {
    fn from(m: user::Model) -> Self {
        Self {
            username: m.username,
            password_hash: m.password_hash,
            is_admin: m.is_admin,
            email: m.email,
            verified: m.verified,
            nickname: m.nickname,
        }
    }
}

/// 按用户名查找
pub async fn find(db: &DatabaseConnection, username: &str) -> Result<Option<User>, AppError> {
    UserEntity::find_by_id(username.to_string())
        .one(db)
        .await
        .map(|opt| opt.map(User::from))
        .map_err(|e| AppError::Internal(format!("查询用户失败: {e}")))
}

/// 认证：校验用户名与密码
///
/// 返回 `Ok(None)` 表示认证失败（不区分用户不存在或密码错误）。
pub async fn authenticate(
    db: &DatabaseConnection,
    username: &str,
    password: &str,
) -> Result<Option<User>, AppError> {
    let Some(u) = find(db, username).await? else {
        return Ok(None);
    };

    let ok = bcrypt::verify(password, &u.password_hash).unwrap_or(false);
    Ok(if ok { Some(u) } else { None })
}

/// 邮箱是否已被占用（空邮箱不参与判定）
pub async fn email_taken(
    db: &DatabaseConnection,
    email: &str,
    except: Option<&str>,
) -> Result<bool, AppError> {
    let email = email.trim();
    if email.is_empty() {
        return Ok(false);
    }

    let mut q = UserEntity::find().filter(user::COLUMN.email.eq(email));
    if let Some(ex) = except {
        q = q.filter(user::COLUMN.username.ne(ex));
    }

    let count = q
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询邮箱失败: {e}")))?;

    Ok(count > 0)
}

/// 新增用户（密码已在调用方哈希好）
pub async fn create(
    db: &DatabaseConnection,
    username: &str,
    password_hash: &str,
    is_admin: bool,
    email: &str,
    verified: bool,
    nickname: &str,
) -> Result<(), AppError> {
    let am = user::ActiveModel {
        username: Set(username.to_string()),
        password_hash: Set(password_hash.to_string()),
        is_admin: Set(is_admin),
        email: Set(email.to_string()),
        verified: Set(verified),
        nickname: Set(nickname.to_string()),
        ..Default::default()
    };

    am.insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("创建用户失败: {e}")))?;
    Ok(())
}

/// 更新用户的部分字段
///
/// 用一个闭包修改 ActiveModel，避免为每种组合写一个函数。
pub async fn update<F>(
    db: &DatabaseConnection,
    username: &str,
    f: F,
) -> Result<bool, AppError>
where
    F: FnOnce(&mut user::ActiveModel),
{
    let Some(model) = UserEntity::find_by_id(username.to_string())
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询用户失败: {e}")))?
    else {
        return Ok(false);
    };

    let mut am: user::ActiveModel = model.into();
    f(&mut am);
    am.update(db)
        .await
        .map_err(|e| AppError::Internal(format!("更新用户失败: {e}")))?;
    Ok(true)
}

/// 首次启动时确保至少有一个管理员
///
/// 返回是否新建了管理员。
///
/// 关键机制：
///   1. 表里已有任何管理员则直接返回；
///   2. 否则用 `.env` 的 ADMIN_USERNAME / ADMIN_PASSWORD 创建；
///   3. `ADMIN_PASSWORD` 为空时随机生成并打印到日志（只能看日志获取）。
pub async fn ensure_admin(db: &DatabaseConnection) -> Result<bool, AppError> {
    let admin_count = UserEntity::find()
        .filter(user::COLUMN.is_admin.eq(true))
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计管理员失败: {e}")))?;

    if admin_count > 0 {
        return Ok(false);
    }

    let username = std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let raw_password = std::env::var("ADMIN_PASSWORD").unwrap_or_default();

    let (password, generated) = if raw_password.trim().is_empty() {
        (random_password(), true)
    } else {
        (raw_password, false)
    };

    let hash = bcrypt::hash(&password, 10)
        .map_err(|e| AppError::Internal(format!("密码哈希失败: {e}")))?;

    create(db, &username, &hash, true, "", true, "").await?;

    if generated {
        log::warn!(
            "已创建管理员 {username}，随机密码：{password}（请登录后立即修改）"
        );
    } else {
        log::info!("已按 .env 创建管理员 {username}");
    }

    Ok(true)
}

/// 生成随机密码（16 位）
///
/// 沿用原实现的做法：用两个 UUID v4 的十六进制拼接后截取。
/// 为什么不引入 `rand` crate：uuid v4 本身由操作系统 CSPRNG 生成，
/// 熵足够（122 位），且 `uuid` 已是现有依赖，不为一个函数多引一个库。
pub fn random_password() -> String {
    let hex = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    hex[..16].to_string()
}

/// 当前时间（RFC3339，秒级）
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
