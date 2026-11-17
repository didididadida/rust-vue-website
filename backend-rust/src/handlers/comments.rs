//! 评论接口
//!
//! 逻辑描述：
//!   输入 → 输出：文章 slug + 评论体 → 评论 JSON 响应
//!   关键机制：
//!     1. 业务委托 `services::comment`，本层做参数解析与权限判断；
//!     2. 是否需审核由环境变量 `WANLE_COMMENT_REVIEW` 决定（默认需要）；
//!     3. 未审核评论只对管理员可见。
//!   不变量：
//!     - 公开列表不返回未审核评论（管理员除外）；
//!     - 公开列表不返回邮箱字段。
//!   边界：
//!     - 文章不存在时返回 404，避免为不存在的 slug 攒评论。
//!
//! 依据：
//!   经典问题：内容审核与可见性控制（moderation & visibility）
//!   来源：
//!     1. 本项目原实现 `handlers/comments.rs` 与 `comments.rs`：
//!        原逻辑用 `WANLE_COMMENT_REVIEW` 决定 is_approved 初值，
//!        公开接口只返回已审核的。
//!     2. OWASP 关于敏感字段不得出现在公开响应的要求
//!        https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html
//!   原型结论：审核状态过滤必须发生在服务端，不能只靠前端隐藏。
//!   本处变体：从 JSON 文件迁到表，可见性规则不变。
//!   沿用结论：保留 WANLE_COMMENT_REVIEW 语义；公开列表过滤未审核。
//!   偏离处：管理员列表不再合并论坛回复（论坛已从项目移除）。

use actix_web::{web, HttpRequest, HttpResponse};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::json;

use crate::errors::AppError;
use crate::middleware;
use crate::models::ApiResponse;
use crate::services::{article, comment};

/// 发表评论的请求体
#[derive(Debug, Deserialize)]
pub struct CommentUpsert {
    /// 昵称
    pub author: String,
    /// 正文（纯文本，前端按纯文本渲染）
    pub content: String,
    /// 邮箱（可选，仅管理员可见）
    #[serde(default)]
    pub email: String,
    /// 回复目标评论 id
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// 评论是否需要审核
fn review_required() -> bool {
    match std::env::var("WANLE_COMMENT_REVIEW") {
        Ok(v) => {
            let v = v.trim().to_lowercase();
            // 只有显式写 false/0/no 才视为关闭审核
            !(v == "false" || v == "0" || v == "no")
        }
        Err(_) => true,
    }
}

/// 公开视图：去掉邮箱
fn public_view(c: &comment::Comment) -> serde_json::Value {
    json!({
        "id": c.id,
        "parent_id": c.parent_id,
        "author": c.author,
        "content": c.content,
        "is_approved": c.is_approved,
        "created_at": c.created_at,
    })
}

/// GET /api/articles/{slug}/comments —— 某篇文章的评论
pub async fn list_comments(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();
    let is_admin = middleware::current_user(&req)
        .map(|c| c.is_admin)
        .unwrap_or(false);

    let all = comment::load(db.as_ref(), &slug).await?;

    // 非管理员只能看到已审核的
    let items: Vec<serde_json::Value> = all
        .iter()
        .filter(|c| is_admin || c.is_approved)
        .map(public_view)
        .collect();

    // 评论列表返回 { total, items }（与原实现一致，前端已按此解析）
    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "total": items.len(),
        "items": items,
    }))))
}

/// POST /api/articles/{slug}/comments —— 发表评论
pub async fn create_comment(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    body: web::Json<CommentUpsert>,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();

    if body.content.trim().is_empty() {
        return Err(AppError::BadRequest("评论内容不能为空".to_string()));
    }
    if body.author.trim().is_empty() {
        return Err(AppError::BadRequest("请填写昵称".to_string()));
    }

    // 文章必须存在，避免为不存在的 slug 攒评论
    if article::find(db.as_ref(), &slug).await?.is_none() {
        return Err(AppError::NotFound("文章不存在".to_string()));
    }

    let approved = !review_required();

    let created = comment::add(
        &db,
        &slug,
        body.author.trim(),
        body.email.trim(),
        body.content.trim(),
        body.parent_id.clone(),
        approved,
    )
    .await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(json!({
        "comment": public_view(&created),
        "pending": !approved,
    }))))
}

/// DELETE /api/comments/{slug}/{id} —— 删除评论
pub async fn delete_comment(
    db: web::Data<DatabaseConnection>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let (slug, id) = path.into_inner();

    let removed = comment::remove(db.as_ref(), &slug, &id).await?;
    if !removed {
        return Err(AppError::NotFound("评论不存在".to_string()));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "id": id }))))
}

/// PUT /api/comments/{slug}/{id}/approve —— 审核评论
pub async fn approve_comment(
    db: web::Data<DatabaseConnection>,
    path: web::Path<(String, String)>,
    body: web::Json<serde_json::Value>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let (slug, id) = path.into_inner();

    // 缺省为 true（通过审核）
    let approved = body
        .get("approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let updated = comment::set_approved(db.as_ref(), &slug, &id, approved)
        .await?
        .ok_or_else(|| AppError::NotFound("评论不存在".to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(public_view(&updated))))
}

/// GET /api/comments —— 管理员查看全站评论（含待审核）
pub async fn list_all_comments(
    db: web::Data<DatabaseConnection>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;

    let all = comment::list_all(db.as_ref()).await?;
    let total = all.len();

    // 最多返回 300 条（个人站评论量级小，分页在内存里做）
    let items: Vec<serde_json::Value> = all
        .into_iter()
        .take(300)
        .map(|(slug, c)| {
            json!({
                "slug": slug,
                "id": c.id,
                "parent_id": c.parent_id,
                "author": c.author,
                "email": c.email,
                "content": c.content,
                "is_approved": c.is_approved,
                "created_at": c.created_at,
            })
        })
        .collect();

    // 评论列表返回 { total, items }（与原实现一致，前端已按此解析）
    // total 是截断前的总数，items 最多 300 条
    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "total": total,
        "items": items,
    }))))
}

/// POST /api/comments/batch-delete —— 批量删除
pub async fn batch_delete(
    db: web::Data<DatabaseConnection>,
    body: web::Json<serde_json::Value>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;

    // 评论需要 (slug, id) 二元组定位
    let pairs: Vec<(String, String)> = body
        .get("items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let slug = v.get("slug")?.as_str()?.to_string();
                    let id = v.get("id")?.as_str()?.to_string();
                    Some((slug, id))
                })
                .collect()
        })
        .unwrap_or_default();

    if pairs.is_empty() {
        return Err(AppError::BadRequest("没有选中任何评论".to_string()));
    }

    let mut deleted = Vec::new();
    let mut failed = Vec::new();

    for (slug, id) in pairs {
        match comment::remove(db.as_ref(), &slug, &id).await {
            Ok(true) => deleted.push(json!({ "slug": slug, "id": id })),
            Ok(false) => failed.push(json!({ "slug": slug, "id": id, "error": "评论不存在" })),
            Err(e) => failed.push(json!({ "slug": slug, "id": id, "error": e.to_string() })),
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "deleted": deleted,
        "failed": failed,
    }))))
}
