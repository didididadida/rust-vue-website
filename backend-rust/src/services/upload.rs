//! 图床上传服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：文件名 + 上传者 → `uploads` 表的增删查；文件本身由 handlers 层落盘
//!   关键机制：
//!     1. filename 为主键，天然保证一图一条归属记录；
//!     2. 删除权限判定：上传者本人或管理员；
//!     3. 无归属记录的历史图片按"无主"处理，只有管理员能删。
//!   不变量：
//!     - 记录里的 uploader 非空（无主用空串表示）；
//!     - 删除文件与删除记录的顺序：先删文件再删记录。
//!   边界：
//!     - 文件不存在时删除接口返回 NotFound；
//!     - 无主图片的删除需要管理员权限。
//!
//! 依据：
//!   经典问题：资源归属追踪与授权判定（ownership tracking + authorization）
//!   来源：
//!     1. OWASP Authorization Cheat Sheet
//!        https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html
//!        要点：授权判断必须在服务端做，基于"资源归属 + 主体角色"两个维度。
//!     2. 本项目原实现 `handlers/upload.rs` 的 `may_delete()`：
//!        原注释："先取归属再判权限：文件不存在时直接 404，不泄露归属信息"
//!   原型结论：
//!     1. 归属 + 角色双维度判定是对象级授权的标准做法；
//!     2. 先判存在性再判权限，可以避免通过错误码差异探测资源是否存在。
//!   本处变体：从 rusqlite 手写 SQL 迁到 SeaORM，判定逻辑不变。
//!   沿用结论：保留 `may_delete(是否管理员, 我, 上传者)` 的三参判定。
//!   偏离处：不做"图片被文章引用时禁止删除"的检查——
//!           原实现也没有，会留下失效图片链接，但个人站可接受。

use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use std::collections::HashSet;

use crate::entities::{upload, upload::Entity as UploadEntity};
use crate::errors::AppError;

/// 删除权限判定
///
/// 规则：
///   - 管理员：可删任何图片
///   - 上传者本人：可删自己的图
///   - 无主图片（uploader 为空）：只有管理员可删
pub fn may_delete(is_admin: bool, me: &str, uploader: &str) -> bool {
    if is_admin {
        return true;
    }
    if uploader.is_empty() {
        return false;
    }
    uploader == me
}

/// 记录一次上传
pub async fn record(
    db: &DatabaseConnection,
    filename: &str,
    uploader: &str,
    size: u64,
    created_at: &str,
) -> Result<(), AppError> {
    let am = upload::ActiveModel {
        filename: Set(filename.to_string()),
        uploader: Set(uploader.to_string()),
        size: Set(size as i64),
        created_at: Set(created_at.to_string()),
    };

    am.insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("记录上传失败: {e}")))?;
    Ok(())
}

/// 删除一条上传记录（文件被删后调用）
pub async fn forget(db: &DatabaseConnection, filename: &str) -> Result<(), AppError> {
    UploadEntity::delete_many()
        .filter(upload::COLUMN.filename.eq(filename))
        .exec(db)
        .await
        .map_err(|e| AppError::Internal(format!("删除上传记录失败: {e}")))?;
    Ok(())
}

/// 查单张图片的归属
///
/// `Ok(None)` 表示无记录（历史图片 / 管理员手工放入）。
pub async fn owner(
    db: &DatabaseConnection,
    filename: &str,
) -> Result<Option<String>, AppError> {
    let row = UploadEntity::find_by_id(filename.to_string())
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询归属失败: {e}")))?;

    Ok(row.map(|m| m.uploader))
}

/// 某个上传者传过的全部文件名
pub async fn by_uploader(
    db: &DatabaseConnection,
    uploader: &str,
) -> Result<HashSet<String>, AppError> {
    let rows = UploadEntity::find()
        .filter(upload::COLUMN.uploader.eq(uploader))
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询上传列表失败: {e}")))?;

    Ok(rows.into_iter().map(|m| m.filename).collect())
}

/// 文件名是否安全（防目录穿越）
///
/// 规则：只允许字母、数字、点、下划线、短横线；
/// 不含路径分隔符；不以点开头（排除 `.` / `..` / 隐藏文件）。
///
/// 迁移自原 `content::is_safe_slug`，逻辑保持一致。
pub fn is_safe_filename(name: &str) -> bool {
    if name.is_empty() || name.starts_with('.') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// 当前时间（RFC3339，秒级）
///
/// 迁移自原 `content::now`，供 handlers 层打时间戳用。
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
