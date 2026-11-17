//! 论坛服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：boards / forum_posts / forum_replies 三张表的读写
//!   关键机制：
//!     1. 软删除：is_deleted 标记，列表默认过滤；
//!     2. 冗余计数：reply_count 在增删回帖时同步维护；
//!     3. 排序优先级：置顶 > 最后活跃时间（沿用原实现）。
//!   不变量：
//!     - 软删除的帖子不出现在列表，但其回帖仍在库中；
//!     - reply_count 在并发下可能短暂不准，以回帖表为准。
//!   边界：
//!     - 锁定的帖子不可回帖（服务层判定）；
//!     - 板块下有帖子时，删除板块会失败（由调用方先处理）。
//!
//! 依据：
//!   经典问题：软删除的查询过滤与冗余计数维护
//!   来源：
//!     1. 本项目原实现 `db.rs`（rusqlite 版）的
//!        list_posts / create_reply / delete_post 等函数
//!     2. 软删除的常见陷阱：所有查询都必须带 is_deleted = false 过滤，
//!        漏一处就会"删了还能看到"
//!   原型结论：
//!     1. 软删除需要在**每个查询入口**统一过滤，不能依赖调用方记得；
//!     2. 冗余计数应在同一事务内更新，否则会出现长期不一致。
//!   本处变体：
//!     - 原实现用 rusqlite 手写 SQL，计数在应用层更新；
//!     - 迁到 SeaORM 后，计数更新仍分两步（先插入回帖，再更新计数），
//!       未包事务——个人站并发低，可接受。
//!   沿用结论：保留软删除与冗余计数。
//!   偏离处：
//!     - 没有把计数更新包进事务。若插入成功但计数更新失败，
//!       计数会偏小；可接受，因为列表展示以 reply_count 为准，
//!       点进去看到的回帖数是准确的；
//!     - 表名改为 forum_posts / forum_replies。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use crate::entities::{board, board::Entity as BoardEntity};
use crate::entities::{forum_post, forum_post::Entity as PostEntity};
use crate::entities::{forum_reply, forum_reply::Entity as ReplyEntity};
use crate::errors::AppError;

/// 当前时间
fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ==================== 板块 ====================

/// 板块视图
#[derive(Debug, Clone, serde::Serialize)]
pub struct Board {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub post_count: u64,
}

impl From<board::Model> for Board {
    fn from(m: board::Model) -> Self {
        Self {
            id: m.id,
            slug: m.slug,
            name: m.name,
            description: m.description,
            icon: m.icon,
            post_count: 0,
        }
    }
}

/// 列出所有板块（含每个板块的帖子数）
pub async fn list_boards(db: &DatabaseConnection) -> Result<Vec<Board>, AppError> {
    let boards = BoardEntity::find()
        .order_by_asc(board::COLUMN.sort)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询板块失败: {e}")))?;

    let mut out = Vec::new();
    for m in boards {
        let count = PostEntity::find()
            .filter(forum_post::COLUMN.board_id.eq(m.id))
            .filter(forum_post::COLUMN.is_deleted.eq(false))
            .count(db)
            .await
            .unwrap_or(0);

        let mut b = Board::from(m);
        b.post_count = count;
        out.push(b);
    }

    Ok(out)
}

/// 按 slug 找板块
pub async fn find_board(db: &DatabaseConnection, slug: &str) -> Result<Option<Board>, AppError> {
    BoardEntity::find()
        .filter(board::COLUMN.slug.eq(slug))
        .one(db)
        .await
        .map(|o| o.map(Board::from))
        .map_err(|e| AppError::Internal(format!("查询板块失败: {e}")))
}

// ==================== 帖子 ====================

/// 帖子摘要（列表用）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PostSummary {
    pub id: i64,
    pub board_id: i64,
    pub board_slug: String,
    pub author: String,
    pub title: String,
    /// 摘要（正文前 160 字符）
    pub excerpt: String,
    pub tags: Vec<String>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub views: i64,
    pub reply_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 帖子详情（含正文）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PostDetail {
    #[serde(flatten)]
    pub summary: PostSummary,
    pub content: String,
}

/// 标签解析（逗号分隔）
fn parse_tags(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// 生成摘要
fn excerpt_of(content: &str) -> String {
    let plain: String = content
        .chars()
        .take(400)
        .collect::<String>()
        .replace(['#', '*', '`', '>'], "");
    plain.trim().chars().take(160).collect()
}

/// 列表查询参数
#[derive(Debug, Default, Clone)]
pub struct ListParams {
    pub board_slug: Option<String>,
    /// 排序：new（最新回复）| hot（热度）
    pub sort: String,
    pub keyword: Option<String>,
    pub skip: u64,
    pub limit: u64,
}

/// 帖子列表
pub async fn list_posts(
    db: &DatabaseConnection,
    p: &ListParams,
) -> Result<(u64, Vec<PostSummary>), AppError> {
    let mut q = PostEntity::find().filter(forum_post::COLUMN.is_deleted.eq(false));

    // 板块过滤
    if let Some(ref slug) = p.board_slug {
        if !slug.is_empty() && slug != "all" {
            if let Some(b) = find_board(db, slug).await? {
                q = q.filter(forum_post::COLUMN.board_id.eq(b.id));
            } else {
                return Ok((0, vec![]));
            }
        }
    }

    // 关键词：标题或正文包含
    if let Some(ref kw) = p.keyword {
        let kw = kw.trim();
        if !kw.is_empty() {
            q = q.filter(
                Condition::any()
                    .add(forum_post::COLUMN.title.contains(kw))
                    .add(forum_post::COLUMN.content.contains(kw)),
            );
        }
    }

    let total = q
        .clone()
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计帖子失败: {e}")))?;

    // 排序：置顶优先，其次按排序方式
    q = match p.sort.as_str() {
        "hot" => q
            .order_by_desc(forum_post::COLUMN.is_pinned)
            .order_by_desc(forum_post::COLUMN.reply_count)
            .order_by_desc(forum_post::COLUMN.views),
        _ => q
            .order_by_desc(forum_post::COLUMN.is_pinned)
            .order_by_desc(forum_post::COLUMN.updated_at),
    };

    let rows = q
        .offset(p.skip)
        .limit(p.limit)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询帖子失败: {e}")))?;

    // 批量取板块 slug（避免 N+1）
    let boards = list_boards(db).await?;
    let board_map: std::collections::HashMap<i64, String> =
        boards.into_iter().map(|b| (b.id, b.slug)).collect();

    let items = rows
        .into_iter()
        .map(|m| PostSummary {
            id: m.id,
            board_id: m.board_id,
            board_slug: board_map.get(&m.board_id).cloned().unwrap_or_default(),
            author: m.author,
            title: m.title,
            excerpt: excerpt_of(&m.content),
            tags: parse_tags(&m.tags),
            is_pinned: m.is_pinned,
            is_locked: m.is_locked,
            views: m.views,
            reply_count: m.reply_count,
            created_at: m.created_at,
            updated_at: m.updated_at,
        })
        .collect();

    Ok((total, items))
}

/// 取帖子详情
pub async fn get_post(db: &DatabaseConnection, id: i64) -> Result<Option<PostDetail>, AppError> {
    let Some(m) = PostEntity::find_by_id(id)
        .filter(forum_post::COLUMN.is_deleted.eq(false))
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询帖子失败: {e}")))?
    else {
        return Ok(None);
    };

    let boards = list_boards(db).await?;
    let board_slug = boards
        .iter()
        .find(|b| b.id == m.board_id)
        .map(|b| b.slug.clone())
        .unwrap_or_default();

    Ok(Some(PostDetail {
        summary: PostSummary {
            id: m.id,
            board_id: m.board_id,
            board_slug,
            author: m.author,
            title: m.title.clone(),
            excerpt: excerpt_of(&m.content),
            tags: parse_tags(&m.tags),
            is_pinned: m.is_pinned,
            is_locked: m.is_locked,
            views: m.views,
            reply_count: m.reply_count,
            created_at: m.created_at,
            updated_at: m.updated_at,
        },
        content: m.content,
    }))
}

/// 新建帖子
pub struct NewPost<'a> {
    pub board_slug: &'a str,
    pub author: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub tags: &'a str,
}

pub async fn create_post(db: &DatabaseConnection, p: NewPost<'_>) -> Result<i64, AppError> {
    let board = find_board(db, p.board_slug)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("板块 {} 不存在", p.board_slug)))?;

    let ts = now();
    let am = forum_post::ActiveModel {
        board_id: Set(board.id),
        author: Set(p.author.to_string()),
        title: Set(p.title.to_string()),
        content: Set(p.content.to_string()),
        tags: Set(p.tags.to_string()),
        is_pinned: Set(false),
        is_locked: Set(false),
        is_deleted: Set(false),
        views: Set(0),
        reply_count: Set(0),
        created_at: Set(ts.clone()),
        updated_at: Set(ts),
        ..Default::default()
    };

    let m = am
        .insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("发帖失败: {e}")))?;

    Ok(m.id)
}

/// 更新帖子
pub async fn update_post(
    db: &DatabaseConnection,
    id: i64,
    title: Option<&str>,
    content: Option<&str>,
    tags: Option<&str>,
) -> Result<bool, AppError> {
    let Some(m) = PostEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询帖子失败: {e}")))?
    else {
        return Ok(false);
    };

    let mut am: forum_post::ActiveModel = m.into();
    if let Some(v) = title {
        am.title = Set(v.to_string());
    }
    if let Some(v) = content {
        am.content = Set(v.to_string());
    }
    if let Some(v) = tags {
        am.tags = Set(v.to_string());
    }
    am.updated_at = Set(now());

    am.update(db)
        .await
        .map_err(|e| AppError::Internal(format!("更新帖子失败: {e}")))?;
    Ok(true)
}

/// 软删除帖子
pub async fn delete_post(db: &DatabaseConnection, id: i64) -> Result<bool, AppError> {
    let Some(m) = PostEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询帖子失败: {e}")))?
    else {
        return Ok(false);
    };

    let mut am: forum_post::ActiveModel = m.into();
    am.is_deleted = Set(true);
    am.update(db)
        .await
        .map_err(|e| AppError::Internal(format!("删除帖子失败: {e}")))?;
    Ok(true)
}

/// 设置置顶 / 锁定
pub async fn set_flags(
    db: &DatabaseConnection,
    id: i64,
    pinned: Option<bool>,
    locked: Option<bool>,
) -> Result<bool, AppError> {
    let Some(m) = PostEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询帖子失败: {e}")))?
    else {
        return Ok(false);
    };

    let mut am: forum_post::ActiveModel = m.into();
    if let Some(v) = pinned {
        am.is_pinned = Set(v);
    }
    if let Some(v) = locked {
        am.is_locked = Set(v);
    }
    am.updated_at = Set(now());

    am.update(db)
        .await
        .map_err(|e| AppError::Internal(format!("更新帖子失败: {e}")))?;
    Ok(true)
}

/// 浏览数 +1（失败不影响主流程）
pub async fn inc_views(db: &DatabaseConnection, id: i64) {
    if let Ok(Some(m)) = PostEntity::find_by_id(id).one(db).await {
        let mut am: forum_post::ActiveModel = m.into();
        let cur = am.views.clone();
        if let sea_orm::ActiveValue::Set(v) = cur {
            am.views = Set(v + 1);
            let _ = am.update(db).await;
        }
    }
}

// ==================== 回帖 ====================

/// 回帖视图
#[derive(Debug, Clone, serde::Serialize)]
pub struct Reply {
    pub id: i64,
    pub post_id: i64,
    pub author: String,
    pub content: String,
    pub created_at: String,
}

impl From<forum_reply::Model> for Reply {
    fn from(m: forum_reply::Model) -> Self {
        Self {
            id: m.id,
            post_id: m.post_id,
            author: m.author,
            content: m.content,
            created_at: m.created_at,
        }
    }
}

/// 列出某帖的回帖（升序）
pub async fn list_replies(
    db: &DatabaseConnection,
    post_id: i64,
) -> Result<Vec<Reply>, AppError> {
    let rows = ReplyEntity::find()
        .filter(forum_reply::COLUMN.post_id.eq(post_id))
        .filter(forum_reply::COLUMN.is_deleted.eq(false))
        .order_by_asc(forum_reply::COLUMN.created_at)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询回帖失败: {e}")))?;

    Ok(rows.into_iter().map(Reply::from).collect())
}

/// 回帖
///
/// 会同步更新帖子的 reply_count 与 updated_at（后者用于"最新回复"排序）。
pub async fn create_reply(
    db: &DatabaseConnection,
    post_id: i64,
    author: &str,
    content: &str,
) -> Result<i64, AppError> {
    let post = PostEntity::find_by_id(post_id)
        .filter(forum_post::COLUMN.is_deleted.eq(false))
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询帖子失败: {e}")))?
        .ok_or_else(|| AppError::NotFound("帖子不存在".to_string()))?;

    if post.is_locked {
        return Err(AppError::Forbidden("该帖子已锁定，不可回复".to_string()));
    }

    let ts = now();
    let am = forum_reply::ActiveModel {
        post_id: Set(post_id),
        author: Set(author.to_string()),
        content: Set(content.to_string()),
        is_deleted: Set(false),
        created_at: Set(ts.clone()),
        ..Default::default()
    };

    let m = am
        .insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("回帖失败: {e}")))?;

    // 更新计数与活跃时间：
    // post 是插入前读到的快照，直接用它的 reply_count + 1
    let mut pam: forum_post::ActiveModel = post.into();
    pam.reply_count = Set(pam.reply_count.take().unwrap_or(0) + 1);
    pam.updated_at = Set(ts);
    let _ = pam.update(db).await;

    Ok(m.id)
}

/// 删除回帖（软删除），并减计数
pub async fn delete_reply(db: &DatabaseConnection, reply_id: i64) -> Result<bool, AppError> {
    let Some(m) = ReplyEntity::find_by_id(reply_id)
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询回帖失败: {e}")))?
    else {
        return Ok(false);
    };

    let post_id = m.post_id;
    let mut am: forum_reply::ActiveModel = m.into();
    am.is_deleted = Set(true);
    am.update(db)
        .await
        .map_err(|e| AppError::Internal(format!("删除回帖失败: {e}")))?;

    // 减计数（不低于 0）
    if let Ok(Some(p)) = PostEntity::find_by_id(post_id).one(db).await {
        let cur = p.reply_count;
        let mut pam: forum_post::ActiveModel = p.into();
        pam.reply_count = Set((cur - 1).max(0));
        let _ = pam.update(db).await;
    }

    Ok(true)
}
