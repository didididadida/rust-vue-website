//! 个人资料存储：`data/profiles/<username>.json`
//!
//! 与 `users.txt` 分开的理由：
//!
//! - `users.txt` 是**认证数据**（密码哈希、角色、验证状态），格式必须稳定，
//!   每一行都是安全敏感的；
//! - 个人资料是**展示数据**（昵称、简介、头像），字段会随界面迭代频繁增加，
//!   放 JSON 里加字段无需改解析逻辑，也方便将来做导入导出。
//!
//! 依据：
//!   经典问题：关注点分离（separation of concerns）/ 变更频率不同的数据不该同表
//!   本处变体：没有数据库，用"两个目录各管一摊"替代两张表
//!   沿用结论：认证格式的稳定性与展示字段的灵活性互不干扰
//!   偏离处：两者不共享事务，删除用户时需手动清理对应 profile 文件

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::content;
use crate::errors::AppError;
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub avatar: String,
    /// 个人主页头图（B 站空间页顶部那张大图）
    #[serde(default)]
    pub banner: String,
    /// 头像裁剪参数：缩放倍数（1.0 = 刚好装下圆框）
    #[serde(default)]
    pub avatar_zoom: f32,
    /// 裁剪偏移（相对图层宽/高的百分比，-50 ~ 50，0 表示居中）
    #[serde(default)]
    pub avatar_x: i16,
    #[serde(default)]
    pub avatar_y: i16,
    /// 注册时间（RFC3339），首次写入时设置
    #[serde(default)]
    pub joined_at: String,
}

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn dir() -> PathBuf {
    paths::data_dir().join("profiles")
}

fn file_path(username: &str) -> Result<PathBuf, AppError> {
    if !content::is_safe_slug(username) {
        return Err(AppError::BadRequest(format!("非法用户名: {}", username)));
    }
    Ok(dir().join(format!("{}.json", username.trim())))
}

/// 读取资料；不存在返回默认值（不报错——老用户从没填过资料是正常的）
///
/// 顺带把裁剪参数归一到合法区间：
///   老资料的 JSON 里没有这些字段（反序列化成 0），必须在读取时就补默认值，
///   否则接口会下发 zoom=0（图片缩成一个点），前端再兜底也容易两边漂移。
pub fn load(username: &str) -> Profile {
    let Ok(path) = file_path(username) else {
        return Profile {
            avatar_zoom: AVATAR_ZOOM_MIN,
            ..Profile::default()
        };
    };
    let mut p = match fs::read_to_string(&path) {
        Ok(raw) if !raw.trim().is_empty() => serde_json::from_str(&raw).unwrap_or_default(),
        _ => Profile::default(),
    };
    p.avatar_zoom = clamp_zoom(p.avatar_zoom);
    p.avatar_x = clamp_offset(p.avatar_x);
    p.avatar_y = clamp_offset(p.avatar_y);
    p
}

/// 写入资料（整体覆盖）
pub fn save(username: &str, profile: &Profile) -> Result<(), AppError> {
    let _guard = lock().lock().unwrap_or_else(|e| e.into_inner());
    let path = file_path(username)?;
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let text = serde_json::to_string_pretty(profile)
        .map_err(|e| AppError::Internal(format!("资料序列化失败: {}", e)))?;
    fs::write(&path, format!("{}\n", text))?;
    Ok(())
}

/// 确保资料存在，不存在则创建并记录注册时间
pub fn ensure(username: &str) -> Profile {
    let mut p = load(username);
    if p.joined_at.is_empty() {
        p.joined_at = content::now();
        let _ = save(username, &p);
    }
    p
}

/// 头像裁剪缩放区间：1.0 = 图片刚好装下圆框；上限 4 倍足够裁出局部
pub const AVATAR_ZOOM_MIN: f32 = 1.0;
pub const AVATAR_ZOOM_MAX: f32 = 4.0;
/// 裁剪偏移区间（百分比）
pub const AVATAR_OFFSET_MAX: i16 = 50;

/// 归一化裁剪缩放：非有限值或越界都回到 1.0
pub fn clamp_zoom(zoom: f32) -> f32 {
    if !zoom.is_finite() || zoom <= 0.0 {
        return AVATAR_ZOOM_MIN;
    }
    zoom.clamp(AVATAR_ZOOM_MIN, AVATAR_ZOOM_MAX)
}

/// 归一化裁剪偏移（百分比，相对图层宽高）
pub fn clamp_offset(v: i16) -> i16 {
    v.clamp(-AVATAR_OFFSET_MAX, AVATAR_OFFSET_MAX)
}

/// 清洗资料字段：去首尾空白、限制长度
///
/// 返回 (nickname, bio, avatar_url, banner_url)。
/// banner 与 avatar 用同一套 URL 规则（站内相对路径或 http(s)），
/// 长度上限也一样——两者都是"指向一张图"的引用，没有理由区别对待。
pub fn clean(
    nickname: &str,
    bio: &str,
    avatar: &str,
    banner: &str,
) -> (String, String, String, String) {
    let nick = nickname.trim().chars().take(24).collect::<String>();
    let b = bio.trim().chars().take(200).collect::<String>();
    let a = avatar.trim().chars().take(500).collect::<String>();
    let bn = banner.trim().chars().take(500).collect::<String>();
    (nick, b, a, bn)
}

/// 校验图片 URL：只接受站内路径或 http(s)，拒绝 javascript: 之类的伪协议
pub fn safe_image_url(url: &str) -> Option<&str> {
    let u = url.trim();
    if u.is_empty() {
        return None;
    }
    if u.starts_with('/') || u.starts_with("http://") || u.starts_with("https://") {
        Some(u)
    } else {
        None
    }
}
