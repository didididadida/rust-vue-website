//! 个人资料实体
//!
//! 逻辑描述：
//!   输入 → 输出：资料字段 ↔ `profiles` 表的一行
//!   关键机制：以 username 为主键，与 users 一对一
//!   不变量：每条资料都对应一个存在的用户
//!   边界：头像裁剪参数是相对量（百分比），与图片尺寸解耦
//!
//! 依据：
//!   经典问题：一对一关系的存储方式（共享主键 vs 独立自增 + 唯一外键）
//!   来源：SeaORM 2.0 关系写法（has_one / belongs_to）
//!        https://github.com/seaql/sea-orm/blob/db40a19a7bde7be8e1c4b346b1b70d72e87f0245/CLAUDE.md
//!        以及原实现：`data/profiles/<username>.json`，文件名即用户名。
//!   原型结论：共享主键的一对一，天然保证「一用户一资料」且无需额外唯一索引
//!   本处变体：原本是每用户一个 JSON 文件，文件名承载了关联关系
//!   沿用结论：迁到表后仍用 username 作主键，语义与文件方案完全对应
//!   偏离处：头像裁剪用 f32，SQLite 存 REAL；
//!           avatar_zoom 默认 1.0 表示"刚好装下圆框"

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub username: String,

    pub nickname: String,
    pub bio: String,

    /// 头像 URL
    pub avatar: String,

    /// 个人主页头图 URL
    pub banner: String,

    /// 头像裁剪：缩放倍数（1.0 = 刚好装下圆框）
    pub avatar_zoom: f32,

    /// 头像裁剪：水平偏移（相对图宽百分比，-50 ~ 50）
    pub avatar_x: i16,

    /// 头像裁剪：垂直偏移（相对图高百分比，-50 ~ 50）
    pub avatar_y: i16,

    /// 该资料所属用户
    #[sea_orm(belongs_to, from = "username", to = "username")]
    pub user: BelongsTo<super::user::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
