//! 评论接口：数据保存在 `data/comments/<slug>.json`，不使用数据库。

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::comments::{self, Comment};
use crate::content::{self, ArticleRecord};
use crate::errors::AppError;
use crate::middleware;
use crate::models::{
    ApiResponse, CommentAdminView, CommentApprove, CommentListResponse, CommentUpsert, CommentView,
};
use crate::paths;

/// 昵称 / 正文长度上限
const MAX_AUTHOR_LEN: usize = 30;
const MAX_CONTENT_LEN: usize = 2000;

fn to_view(item: &Comment) -> CommentView {
    CommentView {
        id: item.id.clone(),
        parent_id: item.parent_id.clone(),
        author: item.author.clone(),
        content: item.content.clone(),
        is_approved: item.is_approved,
        created_at: item.created_at.clone(),
    }
}

fn to_admin_view(slug: &str, item: &Comment) -> CommentAdminView {
    CommentAdminView {
        slug: slug.to_string(),
        id: item.id.clone(),
        parent_id: item.parent_id.clone(),
        author: item.author.clone(),
        email: item.email.clone(),
        content: item.content.clone(),
        is_approved: item.is_approved,
        created_at: item.created_at.clone(),
    }
}

/// 确认文章存在（草稿只能管理员评论）
fn ensure_article(slug: &str, is_admin: bool) -> Result<(), AppError> {
    let doc = content::find::<ArticleRecord>(&paths::posts_dir(), slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;
    if !doc.data.is_published && !is_admin {
        return Err(AppError::NotFound("文章不存在".to_string()));
    }
    Ok(())
}

/// GET /api/articles/{slug}/comments —— 公开，只返回已通过审核的评论
pub async fn list_comments(
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();
    let is_admin = middleware::current_user(&req)
        .map(|claims| claims.is_admin)
        .unwrap_or(false);
    ensure_article(&slug, is_admin)?;

    let items: Vec<CommentView> = comments::load(&slug)?
        .iter()
        .filter(|c| is_admin || c.is_approved)
        .map(to_view)
        .collect();

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(CommentListResponse {
            total: items.len(),
            items,
        })),
    )
}

/// POST /api/articles/{slug}/comments —— 公开，任何人可评论
pub async fn create_comment(
    path: web::Path<String>,
    body: web::Json<CommentUpsert>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();
    let is_admin = middleware::current_user(&req)
        .map(|claims| claims.is_admin)
        .unwrap_or(false);
    ensure_article(&slug, is_admin)?;

    let body = body.into_inner();

    // 蜜罐命中：静默丢弃，假装成功，避免机器人反复试探
    if body
        .website
        .as_deref()
        .map(str::trim)
        .is_some_and(|v| !v.is_empty())
    {
        log::warn!("疑似垃圾评论（蜜罐命中），已丢弃：slug={}", slug);
        return Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
            "accepted": false,
            "requires_review": false
        }))));
    }

    let author = body.author.trim().to_string();
    let content_text = body.content.trim().to_string();

    if author.is_empty() {
        return Err(AppError::BadRequest("昵称不能为空".to_string()));
    }
    if author.chars().count() > MAX_AUTHOR_LEN {
        return Err(AppError::BadRequest(format!(
            "昵称过长（最多 {} 个字符）",
            MAX_AUTHOR_LEN
        )));
    }
    if content_text.is_empty() {
        return Err(AppError::BadRequest("评论内容不能为空".to_string()));
    }
    if content_text.chars().count() > MAX_CONTENT_LEN {
        return Err(AppError::BadRequest(format!(
            "评论过长（最多 {} 个字符）",
            MAX_CONTENT_LEN
        )));
    }

    // 回复目标必须存在，且不能回复自己
    let parent_id = body
        .parent_id
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    if let Some(pid) = parent_id.as_deref() {
        let exists = comments::load(&slug)?.iter().any(|c| c.id == pid);
        if !exists {
            return Err(AppError::BadRequest("要回复的评论不存在".to_string()));
        }
    }

    // 是否需要先审核：设置 WANLE_COMMENT_REVIEW=1 开启
    let requires_review = std::env::var("WANLE_COMMENT_REVIEW")
        .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);

    let comment = Comment {
        id: comments::new_id(),
        parent_id,
        author,
        email: content::clean_opt(body.email).unwrap_or_default(),
        content: content_text,
        is_approved: !requires_review,
        created_at: content::now(),
    };

    let saved = comments::add(&slug, comment)?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "accepted": true,
        "requires_review": !saved.is_approved,
        "comment": to_view(&saved)
    }))))
}

/// DELETE /api/comments/{slug}/{id} —— 管理员删除
pub async fn delete_comment(
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let (slug, id) = path.into_inner();

    if comments::remove(&slug, &id)? {
        Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "id": id }))))
    } else {
        Err(AppError::NotFound("评论不存在".to_string()))
    }
}

// ==================== 批量删除 ====================

/// 一条待删除评论的定位：文章 slug + 评论 id
#[derive(Debug, Deserialize)]
pub struct CommentKey {
    pub slug: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchDeleteBody {
    pub items: Vec<CommentKey>,
}

/// POST /api/comments/batch-delete —— 批量删除评论（管理员）
///
/// 输入 → 输出：{ items: [{slug, id}] } → { deleted: [...], failed: [{slug, id, error}] }
/// 关键机制：评论挂在文章下，单键 (slug,id) 才能唯一定位，
///           所以请求体用对象数组而不是 slug 数组
/// 不变量：deleted + failed 覆盖所有请求项（按 slug/id 去重后）
/// 边界：单次最多 200 条；空列表直接拒绝
///
/// 依据：
///   经典问题：批量操作的部分失败（partial failure）
///   本处变体：评论是"从属实体"，主键是复合键 (slug,id)，删除即改单个 JSON 文件
///   沿用结论：逐条执行、逐条汇报，一条失败不中断其余
///   偏离处：同一文章的多条评论会重复读改写该文件，N 条评论产生 N 次写；
///           后台单次删除量在百级、文件为小 JSON，可接受
pub async fn batch_delete(
    body: web::Json<BatchDeleteBody>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_admin(req)?;
    let body = body.into_inner();

    if body.items.is_empty() {
        return Err(AppError::BadRequest("没有要删除的评论".to_string()));
    }
    if body.items.len() > 200 {
        return Err(AppError::BadRequest("一次最多删除 200 条".to_string()));
    }

    let mut seen = std::collections::HashSet::new();
    let mut deleted: Vec<serde_json::Value> = Vec::new();
    let mut failed: Vec<serde_json::Value> = Vec::new();

    for item in body.items {
        let slug = item.slug.trim().to_string();
        let id = item.id.trim().to_string();
        if slug.is_empty() || id.is_empty() || !seen.insert((slug.clone(), id.clone())) {
            continue;
        }
        match comments::remove(&slug, &id) {
            Ok(true) => deleted.push(json!({ "slug": slug, "id": id })),
            Ok(false) => failed.push(json!({ "slug": slug, "id": id, "error": "评论不存在" })),
            Err(e) => failed.push(json!({ "slug": slug, "id": id, "error": e.message() })),
        }
    }

    log::info!(
        "{} 批量删除评论：成功 {}，失败 {}",
        claims.sub,
        deleted.len(),
        failed.len()
    );
    Ok(HttpResponse::Ok().json(ApiResponse::success(
        json!({ "deleted": deleted, "failed": failed }),
    )))
}

/// PUT /api/comments/{slug}/{id}/approve —— 管理员审核
pub async fn approve_comment(
    path: web::Path<(String, String)>,
    body: web::Json<CommentApprove>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let (slug, id) = path.into_inner();

    match comments::set_approved(&slug, &id, body.approved)? {
        Some(item) => {
            Ok(HttpResponse::Ok().json(ApiResponse::success(to_admin_view(&slug, &item))))
        }
        None => Err(AppError::NotFound("评论不存在".to_string())),
    }
}

/// GET /api/comments —— 管理员查看全站评论（含待审核）
///
/// 输入 → 输出：无 → { total, items: [...] }，按时间倒序
/// 不变量：返回的每条都能定位到 (slug, id)
/// 边界：最多返回 300 条（个人站点评论量级小，分页在内存里做）
pub async fn list_all_comments(req: HttpRequest) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;

    let mut items: Vec<CommentAdminView> = comments::list_all()?
        .iter()
        .map(|(slug, item)| to_admin_view(slug, item))
        .collect();

    // 按时间倒序
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let total = items.len();
    items.truncate(300);

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "total": total,
        "items": items
    }))))
}
