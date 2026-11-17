//! 论坛板块实体
//!
//! 逻辑描述：
//!   输入 → 输出：板块元信息 ↔ `boards` 表的一行
//!   关键机制：slug 唯一且作对外标识；sort 控制展示顺序
//!   不变量：slug 唯一
//!   边界：板块下有帖子时删除需要先迁移或删除帖子（由服务层显式处理）
//!
//! 依据：
//!   经典问题：分类实体的稳定标识
//!   来源：本项目原实现 `db.rs` 的 boards 表定义
//!        （迁移自 rusqlite 手写 SQL，字段一一对应）
//!   原型结论：分类用可读 slug 作对外标识，比自增 id 更适合 URL
//!   本处变体：从手写 SQL 迁到 SeaORM 实体
//!   沿用结论：保留 slug 唯一约束与 sort 排序字段
//!   偏离处：无

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "boards")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    #[sea_orm(unique)]
    pub slug: String,

    pub name: String,
    pub description: String,
    pub icon: String,

    /// 展示顺序（小的在前）
    pub sort: i32,

    pub created_at: String,

    /// 一个板块下有多篇帖子
    #[sea_orm(has_many)]
    pub posts: HasMany<super::forum_post::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
