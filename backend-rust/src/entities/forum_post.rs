//! 论坛帖子实体
//!
//! 逻辑描述：
//!   输入 → 输出：帖子（标题 + 正文 + 板块 + 作者 + 状态） ↔ `forum_posts` 表的一行
//!   关键机制：
//!     1. 通过 `board_id` 归属板块；
//!     2. `is_deleted` 是软删除标记——帖子被删后行保留，避免回帖孤儿；
//!     3. `reply_count` / `views` 是冗余计数，避免每次列表都 COUNT。
//!   不变量：`board_id` 指向存在的板块；软删除的帖子不出现在列表
//!   边界：
//!     - `reply_count` 与实际回帖数可能短暂不一致（并发下），
//!       以回帖表为准，计数字段只用于排序与展示；
//!     - 帖子作者被删后帖子保留（历史记录）。
//!
//! 依据：
//!   经典问题：软删除（soft delete）与冗余计数的取舍
//!   来源：
//!     1. 本项目原实现 `db.rs` 的 posts 表定义
//!        （`is_deleted`、`reply_count`、`views` 字段原样保留）
//!     2. Martin Fowler《冗余计数 / 聚合缓存》
//!        https://martinfowler.com/bliki/CQRS.html 相关讨论中
//!        "denormalized counter" 的常见做法
//!   原型结论：
//!     1. 软删除可以让"删帖"不破坏回帖的外键关系，也便于管理员恢复；
//!     2. 列表页若每次都 COUNT 回帖，会形成 N+1 查询，
//!        冗余计数字段是常见解法。
//!   本处变体：从手写 SQL 迁到 SeaORM；表名加 `forum_` 前缀，
//!             避免与 `articles`（博客文章）概念混淆。
//!   沿用结论：保留 is_deleted 软删除与 reply_count/views 冗余计数。
//!   偏离处：
//!     - 表名从 `posts` 改为 `forum_posts`。原表名在数据库化后
//!       与博客文章（articles）容易混淆，加前缀更清晰；
//!     - 迁移脚本需要把旧 forum.db 的 posts 映射过来。

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "forum_posts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    #[sea_orm(indexed)]
    pub board_id: i64,

    pub author: String,
    pub title: String,
    pub content: String,

    /// 标签，逗号分隔（沿用原实现格式）
    pub tags: String,

    /// 置顶
    pub is_pinned: bool,
    /// 锁定（不可回帖）
    pub is_locked: bool,
    /// 软删除
    pub is_deleted: bool,

    /// 浏览数（冗余计数）
    pub views: i64,
    /// 回帖数（冗余计数）
    pub reply_count: i64,

    pub created_at: String,
    pub updated_at: String,

    /// 帖子所属板块
    #[sea_orm(belongs_to, from = "board_id", to = "id")]
    pub board: BelongsTo<super::board::Entity>,

    /// 一篇帖子有多条回帖
    #[sea_orm(has_many)]
    pub replies: HasMany<super::forum_reply::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
