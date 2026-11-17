//! 评论服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：`comments` 表的读写 —— 按文章取评论、新增、审核、删除、统计
//!   关键机制：
//!     1. 全部功能改为 async，直接走 SeaORM 查询构造器；
//!     2. 楼中楼靠 `parent_id` 自关联，读取时一次性取出整篇评论再在内存组树
//!        （评论量级在百级，不值得用递归 CTE）；
//!     3. 文章 slug 作为归属键，与原文件方案一致。
//!   不变量：
//!     - 返回的评论列表按创建时间升序（老的在上，符合阅读习惯）；
//!     - 删除文章时其评论一并清理（由调用方显式调用 remove_all）。
//!   边界：
//!     - 文章被改名（slug 变化）时，评论需要跟着改，见 `rename_slug`；
//!     - 无评论时返回空 Vec，不是错误。
//!
//! 依据：
//!   经典问题：树形数据的邻接表读取（adjacency list traversal）
//!   来源：
//!     1. SQLite 官方「WITH RECURSIVE」文档
//!        https://sqlite.org/lang_with.html
//!        递归 CTE 可用于取整棵子树，但需要一次查询 + 一次组装。
//!     2. 本项目原实现 `comments.rs`（文件版）：
//!        每篇一个 JSON 数组，靠 parent_id 在前端组树。
//!   原型结论：
//!     1. 邻接表 + 应用层组树，在数据量小时比递归 SQL 更简单；
//!     2. 评论是"一次取全部"的读模式（文章详情页），天然适合整篇取出。
//!   本处变体：从 JSON 文件迁到表；读取方式仍是"按 slug 取全部"。
//!   沿用结论：按 slug 过滤 + 按时间排序，组树交给前端（前端已有该逻辑）。
//!   偏离处：不再有进程内写锁（原文件方案需要 `Mutex` 防止并发读改写文件）；
//!           SQLite 事务本身保证一致性，不需要应用层锁。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entities::{comment, comment::Entity as CommentEntity};
use crate::errors::AppError;

/// 生成评论 id：`c-` + 12 位十六进制
///
/// 沿用原实现的前缀格式，前端未依赖其内部结构。
pub fn new_id() -> String {
    format!("c-{}", uuid::Uuid::new_v4().simple().to_string()[..12].to_string())
}

/// 对外返回的评论结构（与原 JSON 文件方案字段一致）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub email: String,
    pub content: String,
    pub is_approved: bool,
    pub created_at: String,
}

impl From<comment::Model> for Comment {
    fn from(m: comment::Model) -> Self {
        Self {
            id: m.id,
            parent_id: m.parent_id,
            author: m.author,
            email: m.email,
            content: m.content,
            is_approved: m.is_approved,
            created_at: m.created_at,
        }
    }
}

/// 读取某篇文章的全部评论（含未审核的，由调用方决定是否过滤）
pub async fn load(db: &DatabaseConnection, slug: &str) -> Result<Vec<Comment>, AppError> {
    let rows = CommentEntity::find()
        .filter(comment::COLUMN.article_slug.eq(slug))
        .order_by_asc(comment::COLUMN.created_at)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("读取评论失败: {e}")))?;

    Ok(rows.into_iter().map(Comment::from).collect())
}

/// 新增一条评论
pub async fn add(
    db: &DatabaseConnection,
    slug: &str,
    author: &str,
    email: &str,
    content: &str,
    parent_id: Option<String>,
    is_approved: bool,
) -> Result<Comment, AppError> {
    let id = new_id();
    let created_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let am = comment::ActiveModel {
        id: Set(id.clone()),
        article_slug: Set(slug.to_string()),
        parent_id: Set(parent_id.clone()),
        author: Set(author.to_string()),
        email: Set(email.to_string()),
        content: Set(content.to_string()),
        is_approved: Set(is_approved),
        created_at: Set(created_at.clone()),
        ..Default::default()
    };

    am.insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("写入评论失败: {e}")))?;

    Ok(Comment {
        id,
        parent_id,
        author: author.to_string(),
        email: email.to_string(),
        content: content.to_string(),
        is_approved,
        created_at,
    })
}

/// 删除一条评论；返回是否真的删掉了
pub async fn remove(
    db: &DatabaseConnection,
    slug: &str,
    id: &str,
) -> Result<bool, AppError> {
    let res = CommentEntity::delete_many()
        .filter(comment::COLUMN.article_slug.eq(slug))
        .filter(comment::COLUMN.id.eq(id))
        .exec(db)
        .await
        .map_err(|e| AppError::Internal(format!("删除评论失败: {e}")))?;

    Ok(res.rows_affected > 0)
}

/// 审核评论；返回被修改的那条
pub async fn set_approved(
    db: &DatabaseConnection,
    slug: &str,
    id: &str,
    approved: bool,
) -> Result<Option<Comment>, AppError> {
    let found = CommentEntity::find()
        .filter(comment::COLUMN.article_slug.eq(slug))
        .filter(comment::COLUMN.id.eq(id))
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询评论失败: {e}")))?;

    let Some(model) = found else {
        return Ok(None);
    };

    let mut am: comment::ActiveModel = model.into();
    am.is_approved = Set(approved);
    let updated = am
        .update(db)
        .await
        .map_err(|e| AppError::Internal(format!("更新评论失败: {e}")))?;

    Ok(Some(Comment::from(updated)))
}

/// 删除某篇文章的全部评论（文章被删时调用）
pub async fn remove_all(db: &DatabaseConnection, slug: &str) -> Result<bool, AppError> {
    let res = CommentEntity::delete_many()
        .filter(comment::COLUMN.article_slug.eq(slug))
        .exec(db)
        .await
        .map_err(|e| AppError::Internal(format!("清空评论失败: {e}")))?;

    Ok(res.rows_affected > 0)
}

/// 文章 slug 变更时，把评论迁到新 slug
pub async fn rename_slug(
    db: &DatabaseConnection,
    from: &str,
    to: &str,
) -> Result<(), AppError> {
    let rows = CommentEntity::find()
        .filter(comment::COLUMN.article_slug.eq(from))
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询待迁移评论失败: {e}")))?;

    for model in rows {
        let mut am: comment::ActiveModel = model.into();
        am.article_slug = Set(to.to_string());
        am.update(db)
            .await
            .map_err(|e| AppError::Internal(format!("迁移评论失败: {e}")))?;
    }

    Ok(())
}

/// 全站评论（后台审核列表用），按时间倒序
pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<(String, Comment)>, AppError> {
    let rows = CommentEntity::find()
        .order_by_desc(comment::COLUMN.created_at)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("读取全部评论失败: {e}")))?;

    Ok(rows
        .into_iter()
        .map(|m| {
            let slug = m.article_slug.clone();
            (slug, Comment::from(m))
        })
        .collect())
}

/// 全站评论总数
pub async fn count_all(db: &DatabaseConnection) -> Result<u64, AppError> {
    use sea_orm::PaginatorTrait;
    CommentEntity::find()
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计评论失败: {e}")))
}
