//! 数据库连接与 schema 初始化（SeaORM 2.0）
//!
//! 逻辑描述：
//!   输入 → 输出：数据库文件路径 → 已连接的 `DatabaseConnection`，
//!                 并保证所有实体对应的表已存在
//!   关键机制：
//!     1. `Database::connect` 建立带连接池的异步连接（`mode=rwc` 允许自动建库）；
//!     2. `get_schema_registry("wanle_backend::entities::*")` 按实体定义
//!        自动建表/补列（entity-first 工作流），不需要手写 DDL 或 migrations crate；
//!     3. 连接放进 `web::Data` 由 actix 跨 worker 共享。
//!   不变量：
//!     - 表结构与 `src/entities/*.rs` 的字段一一对应；
//!     - 同一数据库文件重复初始化不产生重复表（sync 是幂等的）。
//!   边界：
//!     - schema sync 只做「加表/加列」，**不删列、不删表**——
//!       字段从实体删除后库里的列会残留；
//!     - SQLite 外键默认不开启，级联删除不生效（见下方说明）。
//!
//! 依据：
//!   经典问题：schema 与代码的同步（schema drift）
//!   来源：SeaORM 官方「Entity First Workflow」文档
//!        https://www.sea-ql.org/SeaORM/docs/generate-entity/entity-first/
//!        原文要点：
//!          - "Since 2.0.0"，entity-first 让你手写 entity，SeaORM 生成表与外键；
//!          - "This requires two feature flags `schema-sync` and `entity-registry`"；
//!          - 元路径 "my_crate::entity::*" 必须匹配 Cargo.toml 的 crate 名。
//!        以及 SQLite 官方文档「Foreign Key Support」
//!        https://sqlite.org/foreignkeys.html
//!          —— "Foreign key constraints are disabled by default"
//!   原型结论：
//!     1. Schema Registry 从实体自动推断表结构，需要 schema-sync + entity-registry 特性；
//!     2. SQLite 外键默认关闭，且 `PRAGMA foreign_keys` 是**每连接**生效。
//!   本处变体：
//!     - 单机小站，表结构随代码演进，无多环境迁移需求 → 不引入 sea-orm-migration；
//!     - SeaORM 2.0 的 `Database::connect` 不接受自定义 pool options，
//!       因此无法用 `after_connect` 保证每个新连接都设 PRAGMA。
//!   沿用结论：用 Schema Registry 同步表结构；启动时设 WAL 提升并发读写。
//!   偏离处：**不依赖外键级联**。PRAGMA 只对已建立连接生效，池中新建连接可能未开启，
//!           级联会静默失效。因此业务层（services 层）删除文章时显式清理其评论。

use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

use crate::errors::AppError;
use crate::paths;

/// 建立连接并同步 schema
pub async fn init() -> Result<DatabaseConnection, AppError> {
    let path = paths::data_dir().join("wanle.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // mode=rwc：文件不存在时创建（SQLite URI 选项）
    let url = format!("sqlite://{}?mode=rwc", path.display());

    let db = Database::connect(&url)
        .await
        .map_err(|e| AppError::Internal(format!("数据库连接失败: {e}")))?;

    // WAL：读写不互相阻塞，是本项目并发写入的关键
    // 注意：这是数据库级持久设置，写入后对后续所有连接生效
    db.execute_unprepared("PRAGMA journal_mode=WAL")
        .await
        .map_err(|e| AppError::Internal(format!("设置 WAL 失败: {e}")))?;

    sync_schema(&db).await?;
    Ok(db)
}

/// 按实体定义同步表结构（幂等）
///
/// 关键机制：传入 crate 路径通配符，SeaORM 扫描该路径下所有 entity 并逐个 sync。
/// 路径必须是 `<crate名>::entities::*`，crate 名取自 Cargo.toml 的 `name`
/// （`wanle-backend` → 编译期 crate 名为 `wanle_backend`）。
async fn sync_schema(db: &DatabaseConnection) -> Result<(), AppError> {
    db.get_schema_registry("wanle_backend::entities::*")
        .sync(db)
        .await
        .map_err(|e| AppError::Internal(format!("同步表结构失败: {e}")))?;
    Ok(())
}
