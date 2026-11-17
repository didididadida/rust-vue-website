//! 私信服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：`messages` 表的读写 —— 发消息、列会话、取历史、标记已读、未读数
//!   关键机制：
//!     1. 会话由 `pair_key`（双方用户名有序拼接）唯一确定；
//!     2. 发消息先落库，再由调用方通过 WebSocket Hub 推送——
//!        保证离线用户重连后也能看到完整历史；
//!     3. 会话列表取「每个 pair 的最后一条 + 未读数」。
//!   不变量：
//!     - 不允许给自己发消息；
//!     - `pair_key` 与谁先发无关。
//!   边界：
//!     - 对方用户不存在时拒绝发送（避免产生孤儿消息）；
//!     - 会话列表按最后一条消息时间倒序。
//!
//! 依据：
//!   经典问题：会话聚合（conversation aggregation）与 N+1 查询
//!   来源：
//!     1. 本项目原实现 `db.rs`（rusqlite 版）：
//!        `list_conversations` / `list_conversation` / `send_message` /
//!        `mark_read` / `unread_message_count` 五个函数，
//!        以及 `pair_key(a, b)` 的有序拼接做法。
//!     2. SQLite 官方文档「GROUP BY 与聚合函数」
//!        https://sqlite.org/lang_select.html#resultset
//!   原型结论：
//!     1. 会话列表若对每个 peer 单独查未读数，会产生 N+1 查询；
//!        数据量小时可接受，但更稳的做法是一次取全部未读再在内存分组。
//!     2. 最后一条消息可用 `MAX(created_at)` 配合 GROUP BY 取得。
//!   本处变体：从 rusqlite 手写 SQL 迁到 SeaORM；
//!             数据量（个人站私信）在千条级，用内存分组代替复杂 SQL。
//!   沿用结论：保留 pair_key 与五个操作；会话列表用一次查询 + 内存分组。
//!   偏离处：未做分页的会话列表（个人站会话数很少）；
//!           历史消息做了 limit，避免一次拉太多。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::collections::HashMap;

use crate::entities::{message, message::Entity as MessageEntity};
use crate::errors::AppError;
use crate::services::user;

/// 对外返回的消息结构
#[derive(Debug, Clone, serde::Serialize)]
pub struct Message {
    pub id: i64,
    pub sender: String,
    pub receiver: String,
    pub content: String,
    pub is_read: bool,
    pub created_at: String,
}

impl From<message::Model> for Message {
    fn from(m: message::Model) -> Self {
        Self {
            id: m.id,
            sender: m.sender,
            receiver: m.receiver,
            content: m.content,
            is_read: m.is_read,
            created_at: m.created_at,
        }
    }
}

/// 会话摘要（会话列表的一项）
#[derive(Debug, Clone, serde::Serialize)]
pub struct Conversation {
    /// 对方用户名
    pub peer: String,
    /// 对方展示名
    pub peer_display: String,
    /// 最后一条消息内容（预览用，截断）
    pub last_message: String,
    pub last_at: String,
    /// 我未读的条数
    pub unread: u64,
}

/// 发送一条消息
///
/// 返回落库后的消息；调用方拿到后可通过 WebSocket 推送给对方。
pub async fn send(
    db: &DatabaseConnection,
    sender: &str,
    receiver: &str,
    content: &str,
) -> Result<Message, AppError> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("消息内容不能为空".to_string()));
    }
    if sender == receiver {
        return Err(AppError::BadRequest("不能给自己发消息".to_string()));
    }

    // 对方必须存在，避免产生无法投递的孤儿消息
    if user::find(db, receiver).await?.is_none() {
        return Err(AppError::NotFound("收信人不存在".to_string()));
    }

    let pair_key = message::make_pair_key(sender, receiver);
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let am = message::ActiveModel {
        pair_key: Set(pair_key),
        sender: Set(sender.to_string()),
        receiver: Set(receiver.to_string()),
        content: Set(content.chars().take(2000).collect()),
        is_read: Set(false),
        created_at: Set(now),
        ..Default::default()
    };

    let model = am
        .insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("写入消息失败: {e}")))?;

    Ok(Message::from(model))
}

/// 取与某人的历史消息（按时间升序）
pub async fn history(
    db: &DatabaseConnection,
    me: &str,
    peer: &str,
    limit: u64,
) -> Result<Vec<Message>, AppError> {
    let pair_key = message::make_pair_key(me, peer);

    // 先取最近的 limit 条（倒序），再反转成正序
    let mut rows = MessageEntity::find()
        .filter(message::COLUMN.pair_key.eq(pair_key))
        .order_by_desc(message::COLUMN.created_at)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("读取历史失败: {e}")))?;

    rows.reverse();
    Ok(rows.into_iter().map(Message::from).collect())
}

/// 列出我的全部会话（按最后消息时间倒序）
///
/// 关键机制：一次取出所有与我相关的消息，在内存里按 peer 分组，
/// 避免为每个会话单独查未读数（N+1 查询）。
pub async fn conversations(
    db: &DatabaseConnection,
    me: &str,
) -> Result<Vec<Conversation>, AppError> {
    let rows = MessageEntity::find()
        .filter(Condition::any().add(message::COLUMN.sender.eq(me)).add(message::COLUMN.receiver.eq(me)))
        .order_by_desc(message::COLUMN.created_at)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("读取会话失败: {e}")))?;

    // peer -> (最后一条, 未读数)
    let mut map: HashMap<String, (message::Model, u64)> = HashMap::new();

    // rows 已按时间倒序，所以每个 peer 第一次出现的就是最后一条
    for m in rows {
        let peer = if m.sender == me {
            m.receiver.clone()
        } else {
            m.sender.clone()
        };

        let is_my_unread = m.receiver == me && !m.is_read;

        match map.get_mut(&peer) {
            Some((_, unread)) => {
                if is_my_unread {
                    *unread += 1;
                }
            }
            None => {
                let unread = if is_my_unread { 1 } else { 0 };
                map.insert(peer, (m, unread));
            }
        }
    }

    let mut out = Vec::new();
    for (peer, (last, unread)) in map {
        // 对方展示名：优先昵称，取不到则用用户名
        let peer_display = user::find(db, &peer)
            .await?
            .map(|u| u.display_name().to_string())
            .unwrap_or_else(|| peer.clone());

        out.push(Conversation {
            peer,
            peer_display,
            last_message: last.content.chars().take(60).collect(),
            last_at: last.created_at,
            unread,
        });
    }

    // 按最后消息时间倒序
    out.sort_by(|a, b| b.last_at.cmp(&a.last_at));
    Ok(out)
}

/// 把与某人的消息全部标记为已读
pub async fn mark_read(
    db: &DatabaseConnection,
    me: &str,
    peer: &str,
) -> Result<u64, AppError> {
    let pair_key = message::make_pair_key(me, peer);

    let rows = MessageEntity::find()
        .filter(message::COLUMN.pair_key.eq(pair_key))
        .filter(message::COLUMN.receiver.eq(me))
        .filter(message::COLUMN.is_read.eq(false))
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询未读失败: {e}")))?;

    let mut n = 0u64;
    for model in rows {
        let mut am: message::ActiveModel = model.into();
        am.is_read = Set(true);
        am.update(db)
            .await
            .map_err(|e| AppError::Internal(format!("标记已读失败: {e}")))?;
        n += 1;
    }

    Ok(n)
}

/// 我的总未读数（导航栏红点用）
pub async fn unread_count(db: &DatabaseConnection, me: &str) -> Result<u64, AppError> {
    use sea_orm::PaginatorTrait;

    MessageEntity::find()
        .filter(message::COLUMN.receiver.eq(me))
        .filter(message::COLUMN.is_read.eq(false))
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计未读失败: {e}")))
}
