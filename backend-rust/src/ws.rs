//! 私信 WebSocket
//!
//! 逻辑描述：
//!   输入 → 输出：一个已鉴权的 WS 连接 + 要发给某人的消息 → 送达在线用户；
//!                 离线用户的消息由调用方先落库，重连时拉取
//!   关键机制：
//!     1. 进程内维护 `username -> 发送通道` 的注册表（`Hub`），
//!        每个 WS 连接注册进去，断开时移除；
//!     2. 发消息时先写库（保证不丢），再尝试通过 Hub 实时推送；
//!     3. 心跳：客户端定时发 Ping，服务端回 Pong，超时未收到则断开。
//!   不变量：
//!     - 同一用户可有多条连接（多标签页/多设备），全部保留；
//!     - 消息先落库后推送，推送失败不影响数据一致性。
//!   边界：
//!     - **仅单进程有效**：多实例部署时各进程的连接表互不可见，
//!       此时实时推送只覆盖本进程连接的客户端（个人站单实例，可接受）；
//!     - 客户端断线重连期间的消息不会推送到，需靠拉取接口补齐。
//!
//! 依据：
//!   经典问题：服务器主动推送（server push）与连接注册表
//!   来源：
//!     1. actix 官方 WebSocket 文档
//!        https://github.com/actix/actix-website/blob/22de2c358c8025ff85465d47fa3208a67b2e5839/docs/websockets.md
//!        原文："Actix Web supports a high-level WebSocket interface via the `actix-ws` crate."
//!     2. actix-ws README
//!        https://github.com/actix/actix-extras/blob/663b49670d52a3c6dab67232c63eee1214f4f31a/actix-ws/README.md
//!        标题："WebSockets for Actix Web, without actors."
//!        示例要点：`actix_ws::handle(&req, body)` 返回 (response, session, stream)，
//!                  在 `rt::spawn` 里消费 msg_stream，用 session.text/pong 回写。
//!     3. actix 官方 chat 示例（actor 版）
//!        https://github.com/actix/examples websockets/chat/src/main.rs
//!        定义了 HEARTBEAT_INTERVAL=5s、CLIENT_TIMEOUT=10s。
//!   原型结论：
//!     1. 官方已从 actor 方案转向 actix-ws 的异步流方案，后者更轻；
//!     2. 心跳间隔 5s、超时 10s 是官方示例的经验值；
//!     3. 推送需要一张「用户 → 连接」的注册表，actor 版用 Actor 地址，
//!        async 版用 mpsc 通道。
//!   本处变体：
//!     - 单实例部署，注册表放进程内存即可，不需要 Redis pub/sub；
//!     - 消息必须持久化（用户离线也要能看到），因此先写库再推送。
//!   沿用结论：用 actix-ws 的异步流 + mpsc 通道做注册表；
//!             沿用官方 5s/10s 的心跳参数。
//!   偏离处：不实现"对方正在输入"、已读回执等增强功能——
//!           原实现也没有，且会增加消息类型与状态机复杂度。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix_ws::{Message, MessageStream, Session};
use futures_util::StreamExt;
use tokio::sync::mpsc;

/// 心跳间隔：客户端多久应发一次 Ping
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// 客户端超时：超过这么久没收到任何消息就断开
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// 每个连接的发送通道容量
///
/// 为什么是 32：单个客户端短时间积压的消息条数通常远小于此；
/// 超过说明客户端处理不过来，此时丢弃新消息比无限堆积内存更合理。
const CHANNEL_CAPACITY: usize = 32;

/// 连接注册表
///
/// 一个用户可有多条连接（多个标签页、多台设备），因此 value 是 Vec。
#[derive(Default)]
pub struct Hub {
    inner: Mutex<HashMap<String, Vec<mpsc::Sender<String>>>>,
}

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一条连接，返回接收端供 WS 任务消费
    pub fn register(&self, username: &str) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let mut map = self.inner.lock().expect("hub 锁中毒");
        map.entry(username.to_string()).or_default().push(tx);
        rx
    }

    /// 向指定用户的所有连接推送一条消息
    ///
    /// 返回成功推送的连接数。0 表示对方不在线（消息已落库，重连后可拉取）。
    ///
    /// 关键机制：用 `try_send` 而不是 `send`——
    /// 发送方不应因为某个客户端卡住而阻塞。通道满时丢弃并记日志。
    pub fn push(&self, username: &str, payload: String) -> usize {
        let map = self.inner.lock().expect("hub 锁中毒");
        let Some(senders) = map.get(username) else {
            return 0;
        };

        let mut delivered = 0;
        for tx in senders {
            match tx.try_send(payload.clone()) {
                Ok(_) => delivered += 1,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    log::warn!("用户 {username} 的某个连接积压已满，丢弃一条推送");
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // 连接已关闭，注册表会在 WS 任务结束时清理
                }
            }
        }
        delivered
    }

    /// 清理：把已关闭的通道从注册表移除
    ///
    /// 为什么需要显式清理：WS 任务退出时若只 drop receiver，
    /// sender 仍留在表里，`push` 会一直向死通道写。
    pub fn unregister(&self, username: &str) {
        let mut map = self.inner.lock().expect("hub 锁中毒");
        if let Some(senders) = map.get_mut(username) {
            senders.retain(|tx| !tx.is_closed());
            if senders.is_empty() {
                map.remove(username);
            }
        }
    }

    /// 在线用户数（调试用）
    pub fn online_count(&self) -> usize {
        let map = self.inner.lock().expect("hub 锁中毒");
        map.len()
    }
}

/// 启动一个 WS 会话任务
///
/// 输入：actix 的 Session（写句柄）、MessageStream（读流）、Hub、用户名
/// 输出：任务结束时自动从 Hub 注销
///
/// 关键机制：
///   1. `tokio::select!` 同时等待「客户端消息」与「待推送消息」；
///   2. 心跳超时检查放在收到消息时做（懒检测），避免额外定时器；
///   3. 任一方向结束就退出循环并清理。
pub fn spawn_session(
    mut session: Session,
    mut stream: MessageStream,
    hub: std::sync::Arc<Hub>,
    username: String,
) {
    actix_web::rt::spawn(async move {
        let mut rx = hub.register(&username);
        let mut last_seen = Instant::now();
        log::debug!("WS 连接建立: {username}（在线 {}）", hub.online_count());

        loop {
            tokio::select! {
                // 客户端 → 服务端
                incoming = stream.next() => {
                    match incoming {
                        Some(Ok(Message::Ping(bytes))) => {
                            last_seen = Instant::now();
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            last_seen = Instant::now();
                        }
                        Some(Ok(Message::Text(text))) => {
                            last_seen = Instant::now();
                            // 客户端发来的文本目前只用于心跳之外的无操作；
                            // 私信发送走 HTTP 接口（先落库），不走 WS 入站。
                            log::trace!("WS 收到来自 {username} 的文本: {text}");
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(e)) => {
                            log::debug!("WS 错误（{username}）: {e}");
                            break;
                        }
                        _ => {}
                    }

                    // 懒检测心跳超时
                    if Instant::now().duration_since(last_seen) > CLIENT_TIMEOUT {
                        log::debug!("WS 心跳超时，断开 {username}");
                        break;
                    }
                }

                // 服务端 → 客户端（推送）
                Some(payload) = rx.recv() => {
                    if session.text(payload).await.is_err() {
                        break;
                    }
                }

                // 心跳间隔：主动发 Ping 探测
                _ = tokio::time::sleep(HEARTBEAT_INTERVAL) => {
                    if Instant::now().duration_since(last_seen) > CLIENT_TIMEOUT {
                        log::debug!("WS 心跳超时，断开 {username}");
                        break;
                    }
                    if session.ping(b"").await.is_err() {
                        break;
                    }
                }
            }
        }

        hub.unregister(&username);
        let _ = session.close(None).await;
        log::debug!("WS 连接关闭: {username}（在线 {}）", hub.online_count());
    });
}
