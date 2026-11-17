//! 私信实体
//!
//! 逻辑描述：
//!   输入 → 输出：一条消息 ↔ `messages` 表的一行
//!   关键机制：
//!     - 会话由「双方用户名的有序对」唯一确定，用 `pair_key` 承载；
//!     - 每行同时记录 sender / receiver，便于分页与未读统计；
//!     - `is_read` 标记接收方是否已读，未读数是 `receiver = ? AND is_read = 0` 的计数。
//!   不变量：
//!     - `pair_key` 对同一对人始终相同（与谁先发无关）；
//!     - `sender != receiver`（不允许发给自己）。
//!   边界：
//!     - 用户被删后消息保留（历史记录），查询时按需过滤；
//!     - 不做软删除：删除会话会真正删掉消息（个人站可接受）。
//!
//! 依据：
//!   经典问题：点对点消息的会话建模（conversation modeling）
//!   来源：
//!     1. 本项目原实现 `db.rs` 的 `pair_key(a, b)`：
//!        把两个用户名排序后拼接，保证 A→B 与 B→A 落在同一个键上；
//!        原实现注释："key 为 im:unread:${serverid}" 属另一套（盒子IM 的 Redis 队列），
//!        本项目没有多节点，不需要那层。
//!     2. 盒子IM 的消息模型（Gitee bluexsx/box-im）
//!        https://gitee.com/bluexsx/box-im
//!        它的私聊表同样以「发送者 + 接收者」存储，未读用 Redis 计数。
//!   原型结论：
//!     1. 单机场景下，未读数直接用 SQL `COUNT(*) WHERE receiver=? AND is_read=0`
//!        即可，不需要 Redis 那样的计数器；
//!     2. 会话聚合需要稳定的 pair key，否则同一对话会被拆成两条。
//!   本处变体：从 rusqlite 手写 SQL 迁到 SeaORM 实体；
//!             盒子IM 用 Redis 做跨节点推送，本项目单机 + 内存连接表足够。
//!   沿用结论：保留 pair_key 的有序拼接；未读用 SQL 计数。
//!   偏离处：不记录「已读回执时间」，只记布尔已读——
//!           原实现也只做到未读数，没有逐条回执。

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// 会话键：双方用户名排序后拼接，保证 A→B 与 B→A 相同
    #[sea_orm(indexed)]
    pub pair_key: String,

    pub sender: String,

    #[sea_orm(indexed)]
    pub receiver: String,

    pub content: String,

    /// 接收方是否已读
    pub is_read: bool,

    pub created_at: String,
}

impl ActiveModelBehavior for ActiveModel {}

/// 构造会话键
///
/// 关键机制：把两个用户名按字典序排序后拼接，
/// 使得 (alice, bob) 与 (bob, alice) 得到同一个键。
///
/// 分隔符用 `\u{1}`（不可见控制字符）而不是 `:`，
/// 因为用户名允许含 `:` 时会产生歧义（`a:b` + `c` 与 `a` + `b:c` 撞键）。
pub fn make_pair_key(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}\u{1}{b}")
    } else {
        format!("{b}\u{1}{a}")
    }
}
