//! 数据库实体（SeaORM 2.0 格式）
//!
//! 逻辑描述：
//!   输入 → 输出：无 → 每个表一个 entity 模块，供 handlers / services 层调用
//!   关键机制：SeaORM 2.0 的 `#[sea_orm::model]` 宏，关系直接写在 Model 上
//!   不变量：表名与模块名一一对应；关系方向与业务语义一致
//!   边界：schema 由 `db::init` 调用 Schema Registry 自动创建，本文件只管实体定义
//!
//! 依据：
//!   经典问题：对象-关系映射（ORM）的实体建模
//!   来源：SeaORM 官方项目规范（2.0 格式）
//!        https://github.com/seaql/sea-orm/blob/db40a19a7bde7be8e1c4b346b1b70d72e87f0245/CLAUDE.md
//!        以及 2.0.0 changelog
//!        https://github.com/seaql/sea-orm/blob/db40a19a7bde7be8e1c4b346b1b70d72e87f0245/changelog/2.0.0.md
//!   原型结论：
//!     1. 2.0 用 `#[sea_orm::model]`，关系写在 Model 上，不再有独立的
//!        `Relation` 枚举与 `Related` impl；
//!     2. 强类型列 `COLUMN` 优先于 `Column` 枚举；
//!     3. SQLite 下 `Integer` 与 `BigInteger` 都映射为 `integer`，实体默认 `i64`。
//!   本处变体：本项目从"文件存储"迁到"全量入库"，需要在实体里承载
//!             原本写在 YAML front matter 里的全部字段。
//!   沿用结论：关系用 has_many / belongs_to 直接表达，文章-评论一对多。
//!   偏离处：未使用 SeaORM CLI 生成实体，手写以便保留中文注释；
//!           因此 schema 不由 CLI 校验，改由启动时 Schema Registry 对齐。

pub mod article;
pub mod comment;
pub mod message;
pub mod profile;
pub mod board;
pub mod forum_post;
pub mod forum_reply;
pub mod showcase;
pub mod upload;
pub mod user;
pub mod verification;
