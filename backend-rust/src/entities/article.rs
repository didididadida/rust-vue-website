//! 文章实体
//!
//! 逻辑描述：
//!   输入 → 输出：文章字段 ↔ `articles` 表的一行
//!   关键机制：SeaORM 2.0 的 `#[sea_orm::model]` 声明实体与一对多关系
//!   不变量：`slug` 唯一，是文章的对外标识；`id` 是内部主键
//!   边界：删除文章时其评论由外键级联删除（需 SQLite 开启 foreign_keys）
//!
//! 依据：
//!   经典问题：内容建模中的"稳定标识 vs 可读标识"
//!   来源：SeaORM 2.0 官方实体格式
//!        https://github.com/seaql/sea-orm/blob/db40a19a7bde7be8e1c4b346b1b70d72e87f0245/CLAUDE.md
//!        以及 SQLite 外键默认不启用的事实（SQLite 官方文档 PRAGMA foreign_keys）
//!        https://sqlite.org/foreignkeys.html
//!   原型结论：
//!     1. 对外暴露可读的 slug 比自增 id 更稳定（改标题不影响 URL）；
//!     2. SQLite 的 `PRAGMA foreign_keys` 默认关闭，级联删除不会自动生效。
//!   本处变体：从文件存储迁移而来，slug 原本就是文件名，天然唯一。
//!   沿用结论：slug 加唯一约束并建索引；删除时在应用层显式清理评论。
//!   偏离处：不依赖外键级联，改为服务层显式删除——
//!           因为 SQLite 外键需每连接 PRAGMA，关闭时静默不生效比显式删除更危险。

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// 对外标识（原 Markdown 文件名），唯一
    #[sea_orm(unique)]
    pub slug: String,

    pub title: String,

    /// 摘要，列表页显示
    pub summary: String,

    /// 分类（原 front matter 的 category）
    pub category: String,

    /// 标签，存 JSON 数组字符串（SQLite 无原生数组）
    pub tags: String,

    /// 封面图 URL，可为空
    pub cover_image: Option<String>,

    /// 正文 Markdown
    pub body: String,

    /// 是否发布；false 时仅管理员可见
    pub is_published: bool,

    /// 作者用户名；空串表示本站作者（管理员）
    pub author: String,

    /// 审核状态：approved / pending / rejected
    pub review_status: String,

    /// 是否在首页展示（仅对非本站作者的文章有意义）
    pub is_featured: bool,

    pub created_at: String,
    pub updated_at: String,

    /// 一篇文草有多条评论
    #[sea_orm(has_many)]
    pub comments: HasMany<super::comment::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
