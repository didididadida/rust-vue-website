//! 评论存储：`data/comments/<slug>.json`（纯文本 JSON 文件，不使用数据库）。
//!
//! 每篇文章对应一个文件，文件内容是一个评论数组：
//!
//! ```json
//! [
//!   {
//!     "id": "c-1f3a9b2c4d5e",
//!     "parent_id": null,
//!     "author": "user",
//!     "email": "",
//!     "content": "写得不错！",
//!     "is_approved": true,
//!     "created_at": "2026-10-14T07:20:00Z"
//!   }
//! ]
//! ```
//!
//! 想改成 Markdown 存储时，只需替换 `save()` / `load()` 两个函数即可。

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::content;
use crate::errors::AppError;
use crate::paths;

fn default_true() -> bool {
    true
}

/// 一条评论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    /// 形如 `c-1f3a9b2c4d5e`
    pub id: String,
    /// 回复的目标评论 id；顶层评论为 null
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub author: String,
    /// 邮箱仅存不晒（用于后台联系 / 头像），公开接口不会返回
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub email: String,
    pub content: String,
    #[serde(default = "default_true")]
    pub is_approved: bool,
    #[serde(default)]
    pub created_at: String,
}

/// 写操作串行化，避免多 worker 并发「读-改-写」丢评论
fn write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn file_path(slug: &str) -> Result<PathBuf, AppError> {
    if !content::is_safe_slug(slug) {
        return Err(AppError::BadRequest(format!("非法的文章标识: {}", slug)));
    }
    Ok(paths::comments_dir().join(format!("{}.json", slug.trim())))
}

/// 读取某篇文章的全部评论（按时间正序）
pub fn load(slug: &str) -> Result<Vec<Comment>, AppError> {
    let path = file_path(slug)?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut items: Vec<Comment> = serde_json::from_str(&raw)
        .map_err(|e| AppError::Internal(format!("解析 {} 失败: {}", path.display(), e)))?;
    sort_oldest_first(&mut items);
    Ok(items)
}

fn save(slug: &str, items: &[Comment]) -> Result<(), AppError> {
    let path = file_path(slug)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(items)
        .map_err(|e| AppError::Internal(format!("评论序列化失败: {}", e)))?;
    fs::write(&path, format!("{}\n", text))?;
    log::info!("已写入评论 {}", path.display());
    Ok(())
}

fn sort_oldest_first(items: &mut [Comment]) {
    items.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// 生成一个评论 id
pub fn new_id() -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!("c-{}", &hex[..12])
}

/// 新增一条评论，返回写入后的完整对象
pub fn add(slug: &str, comment: Comment) -> Result<Comment, AppError> {
    let _guard = write_lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut items = load(slug)?;
    items.push(comment.clone());
    save(slug, &items)?;
    Ok(comment)
}

/// 删除一条评论（连同它的直接回复），返回是否真的删掉了东西
pub fn remove(slug: &str, id: &str) -> Result<bool, AppError> {
    let _guard = write_lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut items = load(slug)?;
    let before = items.len();
    items.retain(|c| c.id != id && c.parent_id.as_deref() != Some(id));
    if items.len() == before {
        return Ok(false);
    }
    save(slug, &items)?;
    Ok(true)
}

/// 审核通过 / 取消审核，返回更新后的评论
pub fn set_approved(slug: &str, id: &str, approved: bool) -> Result<Option<Comment>, AppError> {
    let _guard = write_lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut items = load(slug)?;
    let mut updated = None;
    for item in items.iter_mut() {
        if item.id == id {
            item.is_approved = approved;
            updated = Some(item.clone());
        }
    }
    if updated.is_some() {
        save(slug, &items)?;
    }
    Ok(updated)
}

/// 删除某篇文章的全部评论（文章被删除时调用）
pub fn remove_all(slug: &str) -> Result<bool, AppError> {
    let path = file_path(slug)?;
    match fs::remove_file(&path) {
        Ok(_) => {
            log::info!("已删除评论文件 {}", path.display());
            Ok(true)
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// 文章改 slug 时同步迁移评论文件
pub fn rename_slug(from: &str, to: &str) -> Result<(), AppError> {
    if from == to {
        return Ok(());
    }
    let src = file_path(from)?;
    let dst = file_path(to)?;
    if !src.is_file() {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&src, &dst)?;
    log::info!("已迁移评论 {} -> {}", src.display(), dst.display());
    Ok(())
}

/// 列出所有文章的评论（管理员视角），元素为 `(slug, 评论)`
pub fn list_all() -> Result<Vec<(String, Comment)>, AppError> {
    let dir = paths::comments_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut all = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_json = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if !is_json {
            continue;
        }
        let Some(slug) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        for item in load(&slug)? {
            all.push((slug.clone(), item));
        }
    }

    all.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));
    Ok(all)
}

/// 全站评论总数（健康检查用）
pub fn count_all() -> usize {
    list_all().map(|items| items.len()).unwrap_or(0)
}
