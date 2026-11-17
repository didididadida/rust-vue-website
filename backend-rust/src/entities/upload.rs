//! 上传记录实体（图床）
//!
//! 逻辑描述：
//!   输入 → 输出：文件名 + 上传者 + 大小 + 时间 ↔ `uploads` 表的一行
//!   关键机制：filename 作主键，保证同一文件名只有一条归属记录
//!   不变量：uploader 非空；文件被删时记录同步删除
//!   边界：历史图片可能没有记录（早期版本无此表），此时按「无主」处理
//!
//! 依据：
//!   经典问题：对象存储的归属追踪（ownership tracking）
//!   来源：原实现注释与本项目历史行为——
//!         `data/wanle.db` 的 uploads 表，prune 时按 uploader 筛选
//!   原型结论：以资源标识（filename）为主键，比自增 id 更贴合"按名查归属"的读模式
//!   本处变体：从 rusqlite 手写 SQL 迁到 SeaORM 实体
//!   沿用结论：filename 仍是主键，查询走 uploader 索引
//!   偏离处：size 用 i64（SQLite 只有 integer），原实现是 u64 转 i64 存

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "uploads")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub filename: String,

    /// 上传者用户名；空串表示无主（历史图片/管理员手工放入）
    #[sea_orm(indexed)]
    pub uploader: String,

    /// 文件字节数
    pub size: i64,

    pub created_at: String,
}

impl ActiveModelBehavior for ActiveModel {}
