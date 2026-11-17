//! 项目 / 工具实体（共用一张表）
//!
//! 逻辑描述：
//!   输入 → 输出：一条展示项（项目 或 工具） ↔ `showcases` 表的一行
//!   关键机制：
//!     1. 用 `kind` 字段区分 'toy'（自制项目）与 'tool'（实用工具）；
//!     2. 两者共有字段合并（name/description/content/is_published/时间）；
//!     3. 各自特有的字段用可空列承载（项目的 tech_stack/demo_url，工具的分类/url）。
//!   不变量：
//!     - `kind` 只能是 'toy' 或 'tool'；
//!     - `slug` 在同类内唯一（跨类可重名，故唯一约束是 (kind, slug)）
//!   边界：
//!     - 用 kind 区分而非建两张表：两者字段重合度高（约 70%），
//!       且前端列表逻辑几乎相同，合并后少写一套 CRUD；
//!     - 但可空列会让"项目记录里出现 tools 字段"成为可能，需在服务层校验。
//!
//! 依据：
//!   经典问题：单表继承 vs 多表（Single Table Inheritance）
//!   来源：
//!     1. Martin Fowler《Patterns of Enterprise Application Architecture》
//!        中的 "Single Table Inheritance" 模式
//!        https://martinfowler.com/eaaCatalog/singleTableInheritance.html
//!        要点：多个相似类型共享字段时，合并成一张表 + 类型判别列，
//!              可避免多表 JOIN 与重复的 CRUD 代码。
//!     2. 本项目原实现：`content/mod.rs` 的 ToyRecord 与 ToolRecord
//!        两个结构体，字段重合度约 70%（name/description/content/
//!        is_published/created_at/updated_at 完全一致）。
//!   原型结论：
//!     1. 单表继承适合"类型少、字段重合多、查询常混合"的场景；
//!     2. 代价是类型特有字段必须可空，且需要应用层保证不写错。
//!   本处变体：
//!     - 原实现是两套独立的 Markdown 目录（data/toys/ 与 data/tools/），
//!       各自一个结构体；
//!     - 数据库化后，合并成一张表可以减少一半的 CRUD 代码。
//!   沿用结论：采用单表继承，用 kind 判别。
//!   偏离处：
//!     - 唯一约束用 (kind, slug) 复合键而非 slug 单列，
//!       因为项目与工具可以同名（如都叫 "xxx"）；
//!     - 没有强制校验"kind=toy 时 tool 字段为空"，
//!       靠服务层的入参结构体区分（ToyUpsert / ToolUpsert）。

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "showcases")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// 'toy'（自制项目）或 'tool'（实用工具）
    #[sea_orm(indexed)]
    pub kind: String,

    pub slug: String,

    pub name: String,
    pub description: String,

    /// 正文 Markdown（详情页用）
    pub content: String,

    pub is_published: bool,

    pub created_at: String,
    pub updated_at: String,

    // ---------- 项目（kind='toy'）特有 ----------
    pub image: Option<String>,
    pub demo_url: Option<String>,
    pub source_url: Option<String>,
    /// 技术栈，JSON 数组字符串
    pub tech_stack: String,

    // ---------- 工具（kind='tool'）特有 ----------
    pub icon: Option<String>,
    pub category: Option<String>,
    pub url: Option<String>,
}

impl ActiveModelBehavior for ActiveModel {}

/// 类型常量
pub const KIND_TOY: &str = "toy";
pub const KIND_TOOL: &str = "tool";
