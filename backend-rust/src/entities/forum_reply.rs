//! 论坛回帖实体
//!
//! 逻辑描述：
//!   输入 → 输出：回帖（内容 + 作者 + 所属帖子） ↔ `forum_replies` 表的一行
//!   关键机制：
//!     1. 通过 `post_id` 归属帖子；
//!     2. `is_deleted` 软删除，同帖子保持一致；
//!     3. 回帖是平铺的（不支持楼中楼），与原实现一致。
//!   不变量：`post_id` 指向存在的帖子
//!   边界：
//!     - 帖子被软删除时，其回帖仍在库中（恢复帖子可一并恢复）；
//!     - 原实现没有楼中楼，本实体也不引入 parent_id。
//!
//! 依据：
//!   经典问题：一对多关系的子实体建模
//!   来源：本项目原实现 `db.rs` 的 replies 表定义
//!        （字段一一对应，无 parent_id）
//!   原型结论：
//!     1. 平铺回帖适合"楼层"式论坛（每层独立），
//!        与文章评论的"楼中楼"不同；
//!     2. 软删除标记让"删除回帖"不丢失审计信息。
//!   本处变体：从手写 SQL 迁到 SeaORM；表名加 `forum_` 前缀。
//!   沿用结论：保留平铺结构与 is_deleted 软删除。
//!   偏离处：表名从 `replies` 改为 `forum_replies`（避免与文章评论混淆）。

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "forum_replies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    #[sea_orm(indexed)]
    pub post_id: i64,

    pub author: String,
    pub content: String,

    /// 软删除
    pub is_deleted: bool,

    pub created_at: String,

    /// 回帖所属帖子
    #[sea_orm(belongs_to, from = "post_id", to = "id")]
    pub post: BelongsTo<super::forum_post::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
