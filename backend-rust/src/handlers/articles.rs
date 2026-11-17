use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::comments;
use crate::content::{self, ArticleRecord, Doc};
use crate::errors::AppError;
use crate::middleware;
use crate::models::{
    active_category, matches_category, paginate, sort_newest_first, ApiResponse, ArticleUpsert,
    ArticleView,
};
use crate::paths;

/// 列表范围：主页 / 社区 / 我的
#[derive(Debug, Deserialize)]
pub struct ArticleListQuery {
    pub category: Option<String>,
    pub skip: Option<i64>,
    pub limit: Option<i64>,
    /// home（默认）| community | mine
    pub scope: Option<String>,
    /// 按作者过滤
    pub author: Option<String>,
    /// 是否随列表返回正文：缺省 / 任意非 `full` 值 = 只要元数据（列表页）；
    /// `full` = 连同完整 Markdown 正文一起返回（后台编辑等需要正文的调用方）
    pub fields: Option<String>,
}

fn view(slug: String, data: ArticleRecord, body: String) -> ArticleView {
    ArticleView {
        slug,
        title: data.title,
        content: body,
        summary: data.summary,
        category: data.category,
        tags: data.tags,
        cover_image: data.cover_image,
        is_published: data.is_published,
        author: data.author,
        review_status: data.review_status,
        is_featured: data.is_featured,
        created_at: data.created_at,
        updated_at: data.updated_at,
    }
}

fn to_view(doc: Doc<ArticleRecord>) -> ArticleView {
    view(doc.slug, doc.data, doc.body)
}

fn collect() -> Result<Vec<(Doc<ArticleRecord>, ArticleView)>, AppError> {
    Ok(content::list::<ArticleRecord>(&paths::posts_dir())?
        .into_iter()
        .map(|d| {
            let v = to_view(d.clone());
            (d, v)
        })
        .collect())
}

fn resolve_slug(provided: Option<&str>, title: &str) -> Result<String, AppError> {
    match provided.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) if content::is_safe_slug(s) => Ok(s.to_string()),
        Some(s) => Err(AppError::BadRequest(format!("链接别名含非法字符: {}", s))),
        None => Ok(content::slugify(title)),
    }
}

/// 判断某篇文章是否对当前访客可见
///
/// 规则：
/// - 草稿（is_published=false）：只有管理员和作者本人可见
/// - 本站作者文章：已发布即公开
/// - 社区文章：需审核通过才公开；作者本人和管理员始终可见
fn visible_to(rec: &ArticleRecord, username: Option<&str>, is_admin: bool) -> bool {
    let is_owner = username
        .map(|u| {
            rec.author
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case(u))
                .unwrap_or(false)
        })
        .unwrap_or(false);

    if is_admin || is_owner {
        return true;
    }
    if !rec.is_published {
        return false;
    }
    if rec.is_own() {
        return true;
    }
    rec.is_approved()
}

/// GET /api/articles
///
/// scope=home（默认）：本站作者的已发布文章 + 审核通过且被推荐的社区文章
/// scope=community：全部社区文章（审核通过的；管理员另见全部）
/// scope=mine：当前登录用户自己写的全部文章（含草稿与待审核）
pub async fn list_articles(
    query: web::Query<ArticleListQuery>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::current_user(&req);
    let username = claims.as_ref().map(|c| c.sub.clone());
    let is_admin = claims.as_ref().map(|c| c.is_admin).unwrap_or(false);
    let scope = query.scope.as_deref().unwrap_or("home").to_lowercase();

    let all = collect()?;

    let mut picked: Vec<ArticleView> = match scope.as_str() {
        "mine" => {
            let me = username
                .clone()
                .ok_or_else(|| AppError::Unauthorized("请先登录".to_string()))?;
            all.into_iter()
                .filter(|(rec, _)| {
                    rec.data
                        .author
                        .as_deref()
                        .map(|a| a.eq_ignore_ascii_case(&me))
                        .unwrap_or(false)
                        || (rec.data.is_own() && me.eq_ignore_ascii_case("admin"))
                })
                .map(|(_, v)| v)
                .collect()
        }
        "community" => all
            .into_iter()
            .filter(|(rec, _)| {
                // 社区页只展示他人投稿，本站作者的文章留在主页
                !rec.data.is_own() && visible_to(&rec.data, username.as_deref(), is_admin)
            })
            .map(|(_, v)| v)
            .collect(),
        _ => all
            .into_iter()
            .filter(|(rec, _)| visible_to(&rec.data, username.as_deref(), is_admin))
            .filter(|(rec, _)| {
                // 主页：本站作者文章全放；社区文章必须"已审核 + 被推荐"
                rec.data.is_own() || rec.data.show_on_home()
            })
            .map(|(_, v)| v)
            .collect(),
    };

    // 按作者过滤
    if let Some(a) = query
        .author
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        picked.retain(|v| {
            v.author
                .as_deref()
                .map(|x| x.eq_ignore_ascii_case(a))
                .unwrap_or(false)
        });
    }

    if let Some(category) = active_category(query.category.as_deref()) {
        picked.retain(|item| matches_category(&item.category, category));
    }

    sort_newest_first(&mut picked);
    let page = paginate(picked, query.skip.unwrap_or(0), query.limit);

    // 列表默认不返回正文：单篇「网站的构建」正文近 70KB，一次列表请求就把它
    // 连带传给所有访客，而列表页只用 summary。需要正文的调用方显式要 fields=full。
    let with_content = crate::models::wants_content(query.fields.as_deref());
    let page = crate::models::list_out(page, with_content, |v| v.without_content());
    Ok(HttpResponse::Ok().json(ApiResponse::success(page)))
}

/// GET /api/articles/{slug}
pub async fn get_article(
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();
    let doc = content::find::<ArticleRecord>(&paths::posts_dir(), &slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    let claims = middleware::current_user(&req);
    let username = claims.as_ref().map(|c| c.sub.clone());
    let is_admin = claims.as_ref().map(|c| c.is_admin).unwrap_or(false);

    if !visible_to(&doc.data, username.as_deref(), is_admin) {
        return Err(AppError::NotFound("文章不存在".to_string()));
    }
    Ok(HttpResponse::Ok().json(ApiResponse::success(to_view(doc))))
}

/// GET /api/articles/{slug}/raw —— 取原始 .md 全文（含 front matter），供纯文本编辑器使用
pub async fn get_raw(path: web::Path<String>, req: HttpRequest) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let slug = path.into_inner();
    let raw = content::read_raw(&paths::posts_dir(), &slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "slug": slug,
        "raw": raw,
    }))))
}

#[derive(Debug, Deserialize)]
pub struct RawBody {
    /// 完整 .md 文本：`---` + YAML + `---` + 正文
    pub raw: String,
    /// 改过的别名（可选）。与原 slug 不同时，保存成功后重命名文件
    #[serde(default)]
    pub slug: Option<String>,
}

/// PUT /api/articles/{slug}/raw —— 原样保存整个 .md（不做结构体往返）
///
/// 输入 → 输出：{raw, slug?} → 校验 YAML 可解析后原样写回磁盘
/// 关键机制：
///   1. 先用 `ArticleRecord` 解析一遍 `raw`，YAML 写坏直接拒绝，文件不动
///   2. 校验通过后原样写入，不做 serde 序列化，保留用户的字段顺序与格式
///   3. 若带来了新 slug 且合法，写入后重命名文件
/// 不变量：失败时磁盘内容保持调用前状态（校验先于写入）
/// 边界：
///   - 缺 front matter / YAML 语法错 / 缺闭合 --- → 400，附具体原因
///   - 新 slug 非法或已被占用 → 400
///
/// 依据：
///   经典问题：先校验后提交（validate-before-commit）与序列化往返的有损性
///   本处变体：用户手写 YAML，经过结构体再序列化会丢字段顺序、注释和格式
///   沿用结论：把"能不能解析"作为写入门槛，同时放弃中间态转换以保真
///   偏离处：不做字段级归一（tags 去重、slug 自动生成交给用户或前端）
pub async fn put_raw(
    path: web::Path<String>,
    body: web::Json<RawBody>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let old_slug = path.into_inner();
    let body = body.into_inner();

    let existing = content::find::<ArticleRecord>(&paths::posts_dir(), &old_slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    // 权限沿用单篇编辑：管理员或作者本人
    let is_owner = existing
        .data
        .author
        .as_deref()
        .map(|a| a.eq_ignore_ascii_case(&claims.sub))
        .unwrap_or(false);
    if !claims.is_admin && !is_owner {
        return Err(AppError::Forbidden("只能编辑自己的文章".to_string()));
    }

    // 关键：先校验这段文本必须能被解析成合法记录，否则写了就等于让文章消失
    content::parse::<ArticleRecord>(&body.raw)
        .map_err(|e| AppError::BadRequest(format!("格式有误，未保存：{}", e.message())))?;

    let new_slug = body
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != old_slug);

    if let Some(s) = new_slug {
        if !content::is_safe_slug(s) {
            return Err(AppError::BadRequest(format!("别名含非法字符: {}", s)));
        }
        if content::find::<ArticleRecord>(&paths::posts_dir(), s)?.is_some() {
            return Err(AppError::BadRequest(format!("别名已存在: {}", s)));
        }
    }

    // 一律先写原文件；改名时再把内容写到新文件并删掉旧文件
    match new_slug {
        Some(s) => {
            content::write_raw(&paths::posts_dir(), s, &body.raw)?;
            let _ = content::remove(&paths::posts_dir(), &old_slug);
            let _ = comments::rename_slug(&old_slug, s);
            log::info!("{} 原样保存并改名 {} -> {}", claims.sub, old_slug, s);
            Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "slug": s }))))
        }
        None => {
            content::write_raw(&paths::posts_dir(), &old_slug, &body.raw)?;
            log::info!("{} 原样保存 {}", claims.sub, old_slug);
            Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "slug": old_slug }))))
        }
    }
}

/// POST /api/articles
///
/// 管理员创建的视为本站文章（author 空、直接通过）；
/// 普通用户创建的标记 author、置为待审核。
pub async fn create_article(
    body: web::Json<ArticleUpsert>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let body = body.into_inner();

    let title = body.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::BadRequest("标题不能为空".to_string()));
    }
    if title.chars().count() > 120 {
        return Err(AppError::BadRequest("标题最多 120 字".to_string()));
    }

    let dir = paths::posts_dir();
    let slug = resolve_slug(body.slug.as_deref(), &title)?;
    if content::exists(&dir, &slug) {
        return Err(AppError::Conflict(format!(
            "已存在同名文章（{}.md），请更换链接别名",
            slug
        )));
    }

    let now = content::now();
    let is_admin = claims.is_admin;

    let record = ArticleRecord {
        title,
        summary: content::clean_opt(body.summary),
        category: content::clean_opt(body.category),
        tags: content::clean_list(body.tags),
        cover_image: content::clean_opt(body.cover_image),
        // 管理员发布即公开；普通用户发布即视为已发布但待审核
        is_published: body.is_published.unwrap_or(true),
        author: if is_admin {
            None
        } else {
            Some(claims.sub.clone())
        },
        review_status: if is_admin {
            "approved".to_string()
        } else {
            "pending".to_string()
        },
        is_featured: false,
        created_at: now.clone(),
        updated_at: now,
    };
    let markdown = body.content.unwrap_or_default();
    content::save(&dir, &slug, &record, &markdown)?;

    if !is_admin {
        log::info!("社区投稿待审核: {} by {}", slug, claims.sub);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(view(slug, record, markdown))))
}

/// PUT /api/articles/{slug}
pub async fn update_article(
    path: web::Path<String>,
    body: web::Json<ArticleUpsert>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let slug = path.into_inner();
    let dir = paths::posts_dir();

    let existing = content::find::<ArticleRecord>(&dir, &slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    // 只有作者本人或管理员能改
    let is_owner = existing
        .data
        .author
        .as_deref()
        .map(|a| a.eq_ignore_ascii_case(&claims.sub))
        .unwrap_or(false);
    if !claims.is_admin && !is_owner {
        return Err(AppError::Forbidden("只能编辑自己的文章".to_string()));
    }

    let body = body.into_inner();
    let title = body.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::BadRequest("标题不能为空".to_string()));
    }
    if title.chars().count() > 120 {
        return Err(AppError::BadRequest("标题最多 120 字".to_string()));
    }

    // 未传 slug 时沿用原文件名，避免已有链接失效
    let new_slug = match body
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) if content::is_safe_slug(s) => s.to_string(),
        Some(s) => return Err(AppError::BadRequest(format!("链接别名含非法字符: {}", s))),
        None => slug.clone(),
    };
    if new_slug != slug && content::exists(&dir, &new_slug) {
        return Err(AppError::Conflict(format!(
            "已存在同名文章（{}.md）",
            new_slug
        )));
    }

    let now = content::now();
    // 普通用户修改已通过的文章 → 重新进入待审核，防止先过审再改成违规内容
    let (status, featured) = if claims.is_admin {
        (
            existing.data.review_status.clone(),
            existing.data.is_featured,
        )
    } else {
        ("pending".to_string(), false)
    };

    let record = ArticleRecord {
        title,
        summary: content::clean_opt(body.summary),
        category: content::clean_opt(body.category),
        tags: content::clean_list(body.tags),
        cover_image: content::clean_opt(body.cover_image),
        is_published: body.is_published.unwrap_or(existing.data.is_published),
        author: existing.data.author.clone(),
        review_status: status,
        is_featured: featured,
        created_at: if existing.data.created_at.is_empty() {
            now.clone()
        } else {
            existing.data.created_at.clone()
        },
        updated_at: now,
    };
    let markdown = body.content.unwrap_or(existing.body);

    content::save(&dir, &new_slug, &record, &markdown)?;
    if new_slug != slug {
        content::remove(&dir, &slug)?;
        let _ = comments::rename_slug(&slug, &new_slug);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(view(new_slug, record, markdown))))
}

/// 删除文章：物理删除 `.md` 文件，并清理其评论。
///
/// 输入 → 输出：slug → 删除 `posts/<slug>.md` 与 `data/comments/<slug>.json`
/// 关键机制：委托 `content::remove` 与 `comments::remove_all`
/// 不变量：删除后文章与它的评论都不再存在
/// 边界：文件不存在返回 NotFound；评论文件的删除失败不影响主流程
fn delete_article_files(slug: &str) -> Result<(), AppError> {
    if content::remove(&paths::posts_dir(), slug)? {
        let _ = comments::remove_all(slug);
        Ok(())
    } else {
        Err(AppError::NotFound("文章不存在".to_string()))
    }
}

/// DELETE /api/articles/{slug}
pub async fn delete_article(
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let slug = path.into_inner();

    let existing = content::find::<ArticleRecord>(&paths::posts_dir(), &slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    let is_owner = existing
        .data
        .author
        .as_deref()
        .map(|a| a.eq_ignore_ascii_case(&claims.sub))
        .unwrap_or(false);
    if !claims.is_admin && !is_owner {
        return Err(AppError::Forbidden("只能删除自己的文章".to_string()));
    }

    delete_article_files(&slug)?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "slug": slug }))))
}

// ==================== 批量删除 ====================

#[derive(Debug, Deserialize)]
pub struct BatchDeleteBody {
    pub slugs: Vec<String>,
}

/// 单条删除的公共逻辑，批量接口逐条复用
fn delete_one(slug: &str, username: &str, is_admin: bool) -> Result<(), AppError> {
    let existing = content::find::<ArticleRecord>(&paths::posts_dir(), slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    let is_owner = existing
        .data
        .author
        .as_deref()
        .map(|a| a.eq_ignore_ascii_case(username))
        .unwrap_or(false);
    if !is_admin && !is_owner {
        return Err(AppError::Forbidden("只能删除自己的文章".to_string()));
    }

    delete_article_files(slug)
}

/// POST /api/articles/batch-delete —— 批量删除
///
/// 输入 → 输出：{ slugs: [...] } → { deleted: [...], failed: [{slug, error}] }
/// 关键机制：逐条复用单条删除的权限判定；一条失败不影响其它条
/// 不变量：返回的 deleted + failed 覆盖所有请求的 slug
/// 边界：单次最多 100 条；重复 slug 会被去重
///
/// 依据：
///   经典问题：批量操作的部分失败（partial failure）
///   本处变体：删的是文件，没有事务可回滚
///   沿用结论：改成"尽量做、逐条汇报"，让管理员看到哪几条没删掉
///   偏离处：不提供整体回滚——已删除的文件无法恢复（已有 deploy 备份兜底）
pub async fn batch_delete(
    body: web::Json<BatchDeleteBody>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let body = body.into_inner();

    if body.slugs.is_empty() {
        return Err(AppError::BadRequest("没有要删除的文章".to_string()));
    }
    if body.slugs.len() > 100 {
        return Err(AppError::BadRequest("一次最多删除 100 篇".to_string()));
    }

    // 去重，避免同一个 slug 被删两次导致第二条报 NotFound
    let mut seen = std::collections::HashSet::new();
    let mut deleted: Vec<String> = Vec::new();
    let mut failed: Vec<serde_json::Value> = Vec::new();

    for slug in body.slugs {
        let slug = slug.trim().to_string();
        if slug.is_empty() || !seen.insert(slug.clone()) {
            continue;
        }
        match delete_one(&slug, &claims.sub, claims.is_admin) {
            Ok(()) => deleted.push(slug),
            Err(e) => failed.push(json!({ "slug": slug, "error": e.message() })),
        }
    }

    log::info!(
        "{} 批量删除文章：成功 {}，失败 {}",
        claims.sub,
        deleted.len(),
        failed.len()
    );
    Ok(HttpResponse::Ok().json(ApiResponse::success(
        json!({ "deleted": deleted, "failed": failed }),
    )))
}

// ==================== 审核（管理员）====================

#[derive(Debug, Deserialize)]
pub struct ReviewBody {
    /// approved / rejected / pending
    pub status: String,
    /// 是否推荐到首页（仅 approved 时生效）
    #[serde(default)]
    pub featured: Option<bool>,
}

/// PUT /api/articles/{slug}/review —— 审核文章
pub async fn review_article(
    path: web::Path<String>,
    body: web::Json<ReviewBody>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let slug = path.into_inner();
    let body = body.into_inner();

    let status = body.status.trim().to_lowercase();
    if !matches!(status.as_str(), "approved" | "rejected" | "pending") {
        return Err(AppError::BadRequest(
            "状态只能是 approved / rejected / pending".to_string(),
        ));
    }

    let dir = paths::posts_dir();
    let doc = content::find::<ArticleRecord>(&dir, &slug)?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    let mut record = doc.data.clone();
    record.review_status = status.clone();
    record.is_featured = if status == "approved" {
        body.featured.unwrap_or(record.is_featured)
    } else {
        false
    };
    record.updated_at = content::now();

    content::save(&dir, &slug, &record, &doc.body)?;
    log::info!(
        "审核文章 {} → {}（推荐首页: {}）",
        slug,
        status,
        record.is_featured
    );

    Ok(HttpResponse::Ok().json(ApiResponse::success(view(slug, record, doc.body))))
}

/// GET /api/articles/review/pending —— 待审核列表（管理员）
pub async fn list_pending(req: HttpRequest) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let mut items: Vec<ArticleView> = collect()?
        .into_iter()
        .filter(|(rec, _)| {
            !rec.data.is_own() && rec.data.review_status.eq_ignore_ascii_case("pending")
        })
        .map(|(_, v)| v)
        .collect();
    sort_newest_first(&mut items);
    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "total": items.len(),
        "items": items,
    }))))
}
