//! 评论实体
//!
//! 逻辑描述：
//!   输入 → 输出：评论字段 ↔ `comments` 表的一行
//!   关键机制：通过 `article_slug` 归属文章；`parent_id` 自关联实现楼中楼
//!   不变量：顶层评论 `parent_id` 为 NULL；回复的 `parent_id` 指向同文章的另一条评论
//!   边界：文章被删时由服务层显式清理其全部评论
//!
//! 依据：
//!   经典问题：树形结构的邻接表模型（adjacency list）
//!   来源：SQLite 官方文档「邻接表」与递归 CTE
//!        https://sqlite.org/lang_with.html#recursive_queries
//!        SeaORM 2.0 自引用关系写法见官方规范 self_ref 段
//!        https://github.com/seaql/sea-orm/blob/db40a19a7bde7be8e1c4b346b1b70d72e87f0245/CLAUDE.md
//!   原型结论：
//!     1. 邻接表（每条记录存 parent_id）是最简单的树形建模，
//!        读整棵树靠递归 CTE 或应用层组装；
//!     2. 本项目的评论层级浅（通常 2-3 层），应用层组装即可，不需递归 SQL。
//!   本处变体：原实现是每篇文章一个 JSON 文件，数组内靠 parent_id 组树。
//!   沿用结论：迁到表后仍用 parent_id，但增加 article_slug 索引以便按文章取。
//!   偏离处：`id` 沿用原来的字符串形式（c-xxxx），避免迁移时改前端契约。

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "comments")]
pub struct Model {
    /// 对外 id，形如 `c-1f3a9b2c4d5e`（沿用文件存储时期的格式）
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// 所属文章 slug
    #[sea_orm(indexed)]
    pub article_slug: String,

    /// 回复目标评论 id；顶层评论为 None
    pub parent_id: Option<String>,

    pub author: String,

    /// 邮箱仅后台可见/联系用，公开接口不返回
    pub email: String,

    pub content: String,

    pub is_approved: bool,

    pub created_at: String,

    /// 多条评论属于同一篇文章
    #[sea_orm(belongs_to, from = "article_slug", to = "slug")]
    pub article: BelongsTo<super::article::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
