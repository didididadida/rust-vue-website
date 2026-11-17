//! 私信接口（HTTP + WebSocket）
//!
//! 逻辑描述：
//!   输入 → 输出：
//!     - HTTP：发消息 / 列会话 / 取历史 / 标记已读 / 未读数
//!     - WebSocket：`/api/ws` 建立长连接，接收实时推送
//!   关键机制：
//!     1. 发消息走 HTTP（先落库，保证不丢），落库后再通过 Hub 推送给在线对方；
//!     2. WebSocket 只做**下行推送**，不做上行发送——
//!        发送必须经过 HTTP 接口才能保证持久化，避免 WS 断开时丢消息；
//!     3. 鉴权：WS 握手时从查询参数取 token（浏览器 WS API 不能自定义 header）。
//!   不变量：
//!     - 任何消息都先写库再推送；
//!     - WS 连接必须携带有效 JWT 才建立。
//!   边界：
//!     - 对方不在线时推送返回 0，消息仍在库里，对方上线后走 HTTP 拉取；
//!     - token 过期后 WS 不会立即断开（握手时校验一次），续期靠重连。
//!
//! 依据：
//!   经典问题：实时推送与消息可靠投递（reliable delivery with push）
//!   来源：
//!     1. actix 官方 WebSocket 文档
//!        https://github.com/actix/actix-website/blob/22de2c358c8025ff85465d47fa3208a67b2e5839/docs/websockets.md
//!     2. actix-ws README
//!        https://github.com/actix/actix-extras/blob/663b49670d52a3c6dab67232c63eee1214f4f31a/actix-ws/README.md
//!        示例：`let (response, session, msg_stream) = actix_ws::handle(&req, body)?;`
//!     3. MDN：WebSocket 构造函数不接受自定义请求头
//!        https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/WebSocket
//!        因此鉴权只能走 URL 查询参数或 Sec-WebSocket-Protocol。
//!   原型结论：
//!     1. 浏览器 WS 无法自定义 header，token 只能放 query；
//!     2. 「先持久化再推送」是保证消息不丢的标准做法——
//!        纯 WS 发送一旦连接断开就丢消息。
//!   本处变体：
//!     - 原实现用 HTTP 轮询（`list_conversations` 等）；
//!     - 现在增加 WS 做下行推送，HTTP 接口保留作为拉取与发送通道。
//!   沿用结论：HTTP 负责写与拉，WS 只负责通知"有新消息"。
//!   偏离处：
//!     - token 放 query 会在服务器访问日志里留下记录。
//!       本项目的 Nginx 日志已关闭 query 记录（见 nginx.conf），风险可控；
//!     - 未实现「对方正在输入」、已读回执等。

use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::errors::AppError;
use crate::middleware;
use crate::models::ApiResponse;
use crate::services::message as svc;
use crate::ws::{self, Hub};

/// 发送消息的请求体
#[derive(Debug, Deserialize)]
pub struct SendBody {
    pub to: String,
    pub content: String,
}

/// GET /api/messages/conversations —— 我的会话列表
pub async fn list_conversations(
    db: web::Data<DatabaseConnection>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let list = svc::conversations(db.as_ref(), &claims.sub).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(list)))
}

/// GET /api/messages/{peer} —— 与某人的历史消息
///
/// 读取的同时把对方发来的消息标记为已读。
pub async fn history(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let peer = path.into_inner();

    let limit: u64 = 100;
    let msgs = svc::history(db.as_ref(), &claims.sub, &peer, limit).await?;

    // 打开会话即视为已读
    let marked = svc::mark_read(db.as_ref(), &claims.sub, &peer).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "peer": peer,
        "items": msgs,
        "marked_read": marked,
    }))))
}

/// POST /api/messages —— 发送消息
///
/// 先落库，再尝试实时推送；推送失败不影响成功返回。
pub async fn send(
    db: web::Data<DatabaseConnection>,
    hub: web::Data<Arc<Hub>>,
    body: web::Json<SendBody>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    let msg = svc::send(db.as_ref(), &claims.sub, &body.to.trim(), &body.content).await?;

    // 推送给收信人：type 固定为 message，前端据此决定如何处理
    let payload = json!({
        "type": "message",
        "data": {
            "id": msg.id,
            "from": msg.sender,
            "content": msg.content,
            "created_at": msg.created_at,
        }
    })
    .to_string();

    let delivered = hub.push(&body.to.trim(), payload);

    Ok(HttpResponse::Created().json(ApiResponse::success(json!({
        "message": msg,
        "delivered": delivered,
    }))))
}

/// POST /api/messages/{peer}/read —— 标记与某人的消息全部已读
pub async fn mark_read(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let peer = path.into_inner();

    let n = svc::mark_read(db.as_ref(), &claims.sub, &peer).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "marked": n }))))
}

/// GET /api/messages/unread —— 我的总未读数（导航红点）
pub async fn unread(
    db: web::Data<DatabaseConnection>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let n = svc::unread_count(db.as_ref(), &claims.sub).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "unread": n }))))
}

/// GET /api/ws?token=... —— WebSocket 长连接（仅下行推送）
///
/// 鉴权：从查询参数取 JWT。
/// 为什么不用 header：浏览器的 WebSocket 构造函数不支持自定义请求头。
pub async fn ws_route(
    req: HttpRequest,
    body: web::Payload,
    hub: web::Data<Arc<Hub>>,
) -> Result<HttpResponse, actix_web::Error> {
    // 从 query 里取 token
    let query = req.query_string();
    let token = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("token="))
        .map(|v| {
            // 简单百分号解码：token 只含 base64url 与点，一般不需要解码
            v.replace("%2E", ".").replace("%2D", "-").replace("%5F", "_")
        });

    let Some(token) = token else {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
            401,
            "缺少 token",
        )));
    };

    // 校验 JWT
    let claims = match middleware::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
                401,
                "token 无效或已过期",
            )));
        }
    };

    let (response, session, stream) = actix_ws::handle(&req, body)?;

    ws::spawn_session(session, stream, hub.get_ref().clone(), claims.sub);

    Ok(response)
}

/// 供 mod.rs 引用，避免 unused 警告
#[allow(dead_code)]
fn _assert_message_type(_m: Message) {}
