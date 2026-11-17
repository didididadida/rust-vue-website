//! 个人资料服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：`profiles` 表的读写 —— 取资料、保存、确保存在、裁剪参数钳制
//!   关键机制：
//!     1. 以 username 为主键，与 users 一对一；
//!     2. 头像裁剪参数（zoom / offset）是相对量，与图片实际尺寸解耦；
//!     3. 无记录时返回默认值而不是错误（"没设置过资料"是正常状态）。
//!   不变量：
//!     - zoom 落在 [1.0, 3.0]，offset 落在 [-50, 50]；
//!     - 图片 URL 只允许站内相对路径或 http(s)。
//!   边界：
//!     - 保存前会做钳制与清洗，脏数据不会被写进库。
//!
//! 依据：
//!   经典问题：输入钳制与白名单校验（input clamping / allowlist validation）
//!   来源：
//!     1. OWASP Input Validation Cheat Sheet
//!        https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html
//!        要点：优先用白名单（allowlist）而非黑名单；对数值做范围钳制。
//!     2. 本项目原实现 `profile.rs`（JSON 文件版）：
//!        `clamp_zoom` / `clamp_offset` / `safe_image_url` 三个函数。
//!   原型结论：
//!     1. 数值范围钳制能防止前端传异常值导致样式崩坏；
//!     2. URL 白名单可挡住 `javascript:` 这类伪协议注入。
//!   本处变体：从每用户一个 JSON 文件迁到表，校验逻辑原样保留。
//!   沿用结论：保留三个钳制/白名单函数，入库前统一执行。
//!   偏离处：无。

use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};

use crate::entities::{profile, profile::Entity as ProfileEntity};
use crate::errors::AppError;

/// 对外返回的资料结构
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Profile {
    pub nickname: String,
    pub bio: String,
    pub avatar: String,
    pub banner: String,
    pub avatar_zoom: f32,
    pub avatar_x: i16,
    pub avatar_y: i16,
}

impl From<profile::Model> for Profile {
    fn from(m: profile::Model) -> Self {
        Self {
            nickname: m.nickname,
            bio: m.bio,
            avatar: m.avatar,
            banner: m.banner,
            avatar_zoom: m.avatar_zoom,
            avatar_x: m.avatar_x,
            avatar_y: m.avatar_y,
        }
    }
}

/// 缩放倍数钳制到 [1.0, 3.0]
///
/// 下限 1.0：再小就会出现空白边；上限 3.0：再大画质明显下降。
pub fn clamp_zoom(zoom: f32) -> f32 {
    if !zoom.is_finite() {
        return 1.0;
    }
    zoom.clamp(1.0, 3.0)
}

/// 偏移钳制到 [-50, 50]（相对图宽高的百分比，±50 即可把图完全移出视野）
pub fn clamp_offset(v: i16) -> i16 {
    v.clamp(-50, 50)
}

/// 图片 URL 白名单：只放行站内相对路径与 http(s)
///
/// 为什么必须有：这些值会进 <img src> / CSS background，
/// 放任 `javascript:` 或 `data:text/html` 会造成脚本执行。
pub fn safe_image_url(url: &str) -> Option<&str> {
    let u = url.trim();
    if u.is_empty() {
        return None;
    }
    if u.starts_with('/') {
        return Some(u);
    }
    if u.starts_with("http://") || u.starts_with("https://") {
        return Some(u);
    }
    None
}

/// 清洗资料：钳制数值、过滤非法 URL
pub fn clean(p: &Profile) -> Profile {
    Profile {
        nickname: p.nickname.trim().chars().take(32).collect(),
        bio: p.bio.trim().chars().take(500).collect(),
        avatar: safe_image_url(&p.avatar).unwrap_or("").to_string(),
        banner: safe_image_url(&p.banner).unwrap_or("").to_string(),
        avatar_zoom: clamp_zoom(p.avatar_zoom),
        avatar_x: clamp_offset(p.avatar_x),
        avatar_y: clamp_offset(p.avatar_y),
    }
}

/// 读取资料；不存在时返回默认值
pub async fn load(db: &DatabaseConnection, username: &str) -> Result<Profile, AppError> {
    let found = ProfileEntity::find_by_id(username.to_string())
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("读取资料失败: {e}")))?;

    Ok(found.map(Profile::from).unwrap_or_default())
}

/// 保存资料（清洗后写入；不存在则新建）
pub async fn save(
    db: &DatabaseConnection,
    username: &str,
    p: &Profile,
) -> Result<(), AppError> {
    let c = clean(p);

    let exists = ProfileEntity::find_by_id(username.to_string())
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询资料失败: {e}")))?;

    match exists {
        Some(model) => {
            let mut am: profile::ActiveModel = model.into();
            am.nickname = Set(c.nickname);
            am.bio = Set(c.bio);
            am.avatar = Set(c.avatar);
            am.banner = Set(c.banner);
            am.avatar_zoom = Set(c.avatar_zoom);
            am.avatar_x = Set(c.avatar_x);
            am.avatar_y = Set(c.avatar_y);
            am.update(db)
                .await
                .map_err(|e| AppError::Internal(format!("更新资料失败: {e}")))?;
        }
        None => {
            let am = profile::ActiveModel {
                username: Set(username.to_string()),
                nickname: Set(c.nickname),
                bio: Set(c.bio),
                avatar: Set(c.avatar),
                banner: Set(c.banner),
                avatar_zoom: Set(c.avatar_zoom),
                avatar_x: Set(c.avatar_x),
                avatar_y: Set(c.avatar_y),
                ..Default::default()
            };
            am.insert(db)
                .await
                .map_err(|e| AppError::Internal(format!("创建资料失败: {e}")))?;
        }
    }

    Ok(())
}
