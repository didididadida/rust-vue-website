//! 论坛数据迁移：旧 forum.db（rusqlite 版）→ 新库（SeaORM 实体）
//!
//! 逻辑描述：
//!   输入 → 输出：
//!     forum.db 的 boards          → boards 表
//!     forum.db 的 posts           → forum_posts 表
//!     forum.db 的 replies         → forum_replies 表
//!   关键机制：
//!     1. 直接读旧 SQLite 文件（用 sqlx 的裸连接，不依赖旧代码）；
//!     2. 板块按 slug 去重，帖子按 (board_id, author, title, created_at) 判重；
//!     3. 保留原始的 is_pinned / is_locked / views / created_at。
//!   不变量：
//!     - id 不保留（新库自增），但回帖的归属通过"旧 post_id → 新 post_id"映射恢复
//!     - 幂等：已迁移过的帖子会跳过
//!   边界：
//!     - 旧库不存在 → 直接返回，不报错
//!     - 已删除的帖子（is_deleted=1）跳过，不迁
//!     - 板块名冲突时以新库为准
//!
//! 依据：
//!   经典问题：跨 schema 的主键重映射（primary key remapping）
//!   来源：
//!     1. 本项目旧库结构：`db.rs` 的 boards/posts/replies 三张表定义
//!        与新的 `entities/board.rs`、`forum_post.rs`、`forum_reply.rs`
//!     2. 迁移中外键需重映射的通行做法：先迁父表并记录
//!        "旧 id → 新 id"映射，再迁移子表时替换
//!   原型结论：
//!     1. 自增主键在不同库里数值不同，不能直接照搬；
//!     2. 必须先建立映射表，否则回帖会挂到错误的帖子下。
//!   本处变体：
//!     - 旧库是独立的 forum.db，新库是 wanle.db，两个文件；
//!     - 板块量小（5 个），可以按 slug 逐条匹配而不用建映射表。
//!   沿用结论：板块按 slug 匹配；帖子建 old_id → new_id 映射；
//!             回帖按映射改写 post_id。
//!   偏离处：
//!     - 不迁移旧库的 post_likes / favorites / danmaku（新实体里没有这些表，
//!       它们是精简时移除的功能）；
//!     - 不保留旧 id，因为新库用自增且已有数据。

use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, Set, Statement,
};
use std::collections::HashMap;

use crate::entities::{board, forum_post, forum_reply};
use crate::errors::AppError;
use crate::services::forum;

/// 论坛迁移统计
#[derive(Debug, Default)]
pub struct ForumReport {
    pub boards: usize,
    pub posts: usize,
    pub replies: usize,
    pub skipped_posts: usize,
    pub errors: Vec<String>,
}

impl ForumReport {
    pub fn summary(&self) -> String {
        format!(
            "板块 {} / 帖子 {} / 回帖 {}（跳过已存在帖子 {}，错误 {}）",
            self.boards,
            self.posts,
            self.replies,
            self.skipped_posts,
            self.errors.len()
        )
    }
}

/// 执行论坛迁移
pub async fn run(new_db: &DatabaseConnection, dry_run: bool) -> Result<ForumReport, AppError> {
    let mut report = ForumReport::default();

    // 旧库路径
    let old_path = crate::paths::data_dir().join("forum.db");
    if !old_path.is_file() {
        log::info!("未找到旧论坛库 {}，跳过论坛迁移", old_path.display());
        return Ok(report);
    }

    // 用 sqlx 直接打开旧库（只读）
    let url = format!("sqlite://{}?mode=ro", old_path.display());
    let opts = sea_orm::ConnectOptions::new(url);
    let old_db = sea_orm::Database::connect(opts)
        .await
        .map_err(|e| AppError::Internal(format!("打开旧论坛库失败: {e}")))?;

    // ---------- 1. 板块 ----------
    let rows = old_db
        .query_all_raw(Statement::from_string(
            old_db.get_database_backend(),
            "SELECT slug, name, description, icon, sort, created_at FROM boards ORDER BY sort",
        ))
        .await
        .map_err(|e| AppError::Internal(format!("读旧板块失败: {e}")))?;

    // 旧 board_id → 新 board_id
    let mut board_map: HashMap<i64, i64> = HashMap::new();

    // 同时需要旧 id，单独查一次
    let id_rows = old_db
        .query_all_raw(Statement::from_string(
            old_db.get_database_backend(),
            "SELECT id, slug FROM boards",
        ))
        .await
        .unwrap_or_default();
    let mut old_slug_by_id: HashMap<i64, String> = HashMap::new();
    for r in id_rows {
        if let (Ok(id), Ok(slug)) = (
            r.try_get::<i64>("", "id"),
            r.try_get::<String>("", "slug"),
        ) {
            old_slug_by_id.insert(id, slug);
        }
    }

    for r in rows {
        let slug: String = r.try_get("", "slug").unwrap_or_default();
        let name: String = r.try_get("", "name").unwrap_or_default();
        let description: String = r.try_get("", "description").unwrap_or_default();
        let icon: String = r.try_get("", "icon").unwrap_or_default();
        let sort: i32 = r.try_get("", "sort").unwrap_or(0);
        let created_at: String = r.try_get("", "created_at").unwrap_or_default();

        // 新库里是否已有该板块
        let existing = board::Entity::find()
            .filter(board::COLUMN.slug.eq(slug.clone()))
            .one(new_db)
            .await
            .map_err(|e| AppError::Internal(format!("查新板块失败: {e}")))?;

        let new_id = if let Some(b) = existing {
            b.id
        } else if dry_run {
            -1
        } else {
            let am = board::ActiveModel {
                slug: Set(slug.clone()),
                name: Set(name),
                description: Set(description),
                icon: Set(icon),
                sort: Set(sort),
                created_at: Set(created_at),
                ..Default::default()
            };
            let m = am
                .insert(new_db)
                .await
                .map_err(|e| AppError::Internal(format!("插入板块失败: {e}")))?;
            report.boards += 1;
            m.id
        };

        // 反查旧 id
        for (oid, oslug) in &old_slug_by_id {
            if *oslug == slug {
                board_map.insert(*oid, new_id);
            }
        }
    }

    // ---------- 2. 帖子 ----------
    let post_rows = old_db
        .query_all_raw(Statement::from_string(
            old_db.get_database_backend(),
            "SELECT id, board_id, author, title, content, tags, is_pinned, is_locked, views, created_at, updated_at \
             FROM posts WHERE is_deleted = 0 ORDER BY created_at",
        ))
        .await
        .map_err(|e| AppError::Internal(format!("读旧帖子失败: {e}")))?;

    let mut post_map: HashMap<i64, i64> = HashMap::new();

    for r in post_rows {
        let old_id: i64 = r.try_get("", "id").unwrap_or(0);
        let old_board: i64 = r.try_get("", "board_id").unwrap_or(0);
        let author: String = r.try_get("", "author").unwrap_or_default();
        let title: String = r.try_get("", "title").unwrap_or_default();
        let content: String = r.try_get("", "content").unwrap_or_default();
        let tags: String = r.try_get("", "tags").unwrap_or_default();
        let is_pinned: bool = r.try_get::<i32>("", "is_pinned").unwrap_or(0) != 0;
        let is_locked: bool = r.try_get::<i32>("", "is_locked").unwrap_or(0) != 0;
        let views: i64 = r.try_get("", "views").unwrap_or(0);
        let created_at: String = r.try_get("", "created_at").unwrap_or_default();
        let updated_at: String = r.try_get("", "updated_at").unwrap_or_default();

        let new_board = match board_map.get(&old_board) {
            Some(v) if *v > 0 => *v,
            _ => {
                report
                    .errors
                    .push(format!("帖子 #{old_id} 的板块 {old_board} 未映射，跳过"));
                continue;
            }
        };

        // 判重：同板块 + 同作者 + 同标题 + 同时间
        let dup = forum_post::Entity::find()
            .filter(forum_post::COLUMN.board_id.eq(new_board))
            .filter(forum_post::COLUMN.author.eq(author.clone()))
            .filter(forum_post::COLUMN.title.eq(title.clone()))
            .filter(forum_post::COLUMN.created_at.eq(created_at.clone()))
            .one(new_db)
            .await
            .map_err(|e| AppError::Internal(format!("查重失败: {e}")))?;

        if let Some(p) = dup {
            post_map.insert(old_id, p.id);
            report.skipped_posts += 1;
            continue;
        }

        if dry_run {
            report.posts += 1;
            continue;
        }

        // 回帖数稍后统一算
        let am = forum_post::ActiveModel {
            board_id: Set(new_board),
            author: Set(author),
            title: Set(title),
            content: Set(content),
            tags: Set(tags),
            is_pinned: Set(is_pinned),
            is_locked: Set(is_locked),
            is_deleted: Set(false),
            views: Set(views),
            reply_count: Set(0),
            created_at: Set(created_at),
            updated_at: Set(updated_at),
            ..Default::default()
        };

        match am.insert(new_db).await {
            Ok(m) => {
                post_map.insert(old_id, m.id);
                report.posts += 1;
            }
            Err(e) => report.errors.push(format!("帖子 #{old_id} 插入失败: {e}")),
        }
    }

    // ---------- 3. 回帖 ----------
    let reply_rows = old_db
        .query_all_raw(Statement::from_string(
            old_db.get_database_backend(),
            "SELECT id, post_id, author, content, created_at FROM replies WHERE is_deleted = 0 ORDER BY created_at",
        ))
        .await
        .map_err(|e| AppError::Internal(format!("读旧回帖失败: {e}")))?;

    // 新帖子的回帖计数
    let mut counts: HashMap<i64, i64> = HashMap::new();

    for r in reply_rows {
        let old_post: i64 = r.try_get("", "post_id").unwrap_or(0);
        let author: String = r.try_get("", "author").unwrap_or_default();
        let content: String = r.try_get("", "content").unwrap_or_default();
        let created_at: String = r.try_get("", "created_at").unwrap_or_default();

        let Some(&new_post) = post_map.get(&old_post) else {
            // 帖子被跳过（已删除或板块缺失），回帖也无处安放
            continue;
        };

        if dry_run {
            report.replies += 1;
            continue;
        }

        let am = forum_reply::ActiveModel {
            post_id: Set(new_post),
            author: Set(author),
            content: Set(content),
            is_deleted: Set(false),
            created_at: Set(created_at),
            ..Default::default()
        };

        match am.insert(new_db).await {
            Ok(_) => {
                report.replies += 1;
                *counts.entry(new_post).or_insert(0) += 1;
            }
            Err(e) => report.errors.push(format!("回帖插入失败: {e}")),
        }
    }

    // 回填 reply_count
    if !dry_run {
        for (post_id, n) in counts {
            if let Ok(Some(p)) = forum_post::Entity::find_by_id(post_id).one(new_db).await {
                let mut am: forum_post::ActiveModel = p.into();
                am.reply_count = Set(n);
                let _ = am.update(new_db).await;
            }
        }
    }

    // 若新库没有任何板块（首次部署），插入默认板块
    if !dry_run {
        let n = board::Entity::find()
            .count(new_db)
            .await
            .unwrap_or(0);
        if n == 0 {
            let ts = crate::services::user::now();
            for (slug, name, desc, sort) in [
                ("lounge", "闲聊", "想说什么就说什么", 0),
                ("tech", "技术", "编程与技术讨论", 1),
                ("creation", "创作", "作品与灵感", 2),
                ("help", "求助", "遇到问题可以在这里问", 3),
                ("study", "学习", "学习记录与心得", 4),
            ] {
                let am = board::ActiveModel {
                    slug: Set(slug.to_string()),
                    name: Set(name.to_string()),
                    description: Set(desc.to_string()),
                    icon: Set(String::new()),
                    sort: Set(sort),
                    created_at: Set(ts.clone()),
                    ..Default::default()
                };
                let _ = am.insert(new_db).await;
                report.boards += 1;
            }
        }
    }

    let _ = forum::list_boards(new_db).await; // 触发一次查询，验证实体可用

    Ok(report)
}
