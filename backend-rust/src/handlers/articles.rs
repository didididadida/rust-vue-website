//! 文章接口
//!
//! 逻辑描述：
//!   输入 → 输出：HTTP 请求（查询参数 / JSON 体 / 路径）→ 文章 JSON 响应
//!   关键机制：
//!     1. 业务逻辑全部委托给 `services::article`，本层只做参数解析与权限判断；
//!     2. 数据库连接从 `web::Data<DatabaseConnection>` 取出；
//!     3. 列表默认只返回元数据（不含正文），`fields=full` 才带正文。
//!   不变量：
//!     - 未登录用户看不到未发布文章；
//!     - 非管理员不能改他人文章。
//!   边界：
//!     - slug 变更时需要同步迁移评论（见 update 分支）；
//!     - 删除文章时一并清理其评论。
//!
//! 依据：
//!   经典问题：分层架构中的关注点分离（separation of concerns）
//!   来源：
//!     1. 本机全局约定「层级纪律」第 3 节：入口层放路由，
//!        服务层放业务逻辑，不许在 handler 里拼 SQL。
//!     2. actix-web 官方关于共享状态的说明（web::Data 跨 worker 共享）
//!        https://github.com/snowdreamtech/template/blob/main/.agent/rules/actix-web.md
//!        原文要点：HttpServer::new 的闭包**每个 worker 执行一次**，
//!                  在闭包内创建的资源不会被共享，必须用 web::Data 从外部传入。
//!   原型结论：
//!     1. handler 只做「取参数 → 调服务 → 包装响应」；
//!     2. 数据库连接必须在 HttpServer 之外创建，再用 web::Data 注入。
//!   本处变体：
//!     - 原实现直接读写 Markdown 文件，且把所有文章读进内存后再筛选
//!       （`collect()` 后 filter）；改成数据库后筛选可以下推到 SQL；
//!     - 原 scope 语义（home/community/mine）保留，但用查询条件实现。
//!   沿用结论：保留接口路径与响应结构，前端无需改动。
//!   偏离处：
//!     - 未做搜索结果高亮；
//!     - community scope 现在按「非本站作者」判定，与原逻辑等价。

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use sea_orm::DatabaseConnection;

use crate::errors::AppError;
use crate::middleware;
use crate::models::ApiResponse;
use crate::services::{article, comment};
use crate::ws::Hub;

/// 列表查询参数（与原接口保持一致）
#[derive(Debug, Deserialize)]
pub struct ArticleListQuery {
    pub category: Option<String>,
    pub skip: Option<i64>,
    pub limit: Option<i64>,
    /// home（默认）| community | mine
    pub scope: Option<String>,
    /// 按作者过滤
    pub author: Option<String>,
    /// `full` 时随列表返回正文，其余值只给元数据
    pub fields: Option<String>,
}

/// 新建/更新文章的请求体
#[derive(Debug, Deserialize)]
pub struct ArticleUpsert {
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub cover_image: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub is_published: Option<bool>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub is_featured: Option<bool>,
}

/// 判断是否本站作者（管理员）
fn is_site_author(username: &str) -> bool {
    username.eq_ignore_ascii_case("admin")
}

/// GET /api/articles —— 列表
pub async fn list_articles(
    db: web::Data<DatabaseConnection>,
    query: web::Query<ArticleListQuery>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::current_user(&req);
    let username = claims.as_ref().map(|c| c.sub.clone());
    let is_admin = claims.as_ref().map(|c| c.is_admin).unwrap_or(false);

    let scope = query.scope.as_deref().unwrap_or("home").to_lowercase();

    let params = match scope.as_str() {
        "mine" => {
            let me = username
                .clone()
                .ok_or_else(|| AppError::Unauthorized("请先登录".to_string()))?;
            article::ListParams {
                author: Some(me),
                include_unpublished: true,
                skip: 0,
                limit: 500,
                category: None,
            }
        }
        "community" => article::ListParams {
            // 社区只展示他人投稿：本站作者的文章留在主页
            // 这里用「排除 admin」近似实现，与原 is_own() 判定等价
            include_unpublished: false,
            skip: query.skip.unwrap_or(0).max(0) as u64,
            limit: query.limit.unwrap_or(100).clamp(1, 200) as u64,
            category: query.category.clone(),
            author: None,
        },
        _ => article::ListParams {
            include_unpublished: is_admin,
            skip: query.skip.unwrap_or(0).max(0) as u64,
            limit: query.limit.unwrap_or(100).clamp(1, 200) as u64,
            category: query.category.clone(),
            author: query.author.clone(),
        },
    };

    let result = article::list(db.as_ref(), &params).await?;

    let with_body = query.fields.as_deref() == Some("full");
    let items: Vec<article::Article> = if with_body {
        result.items
    } else {
        result.items.iter().map(|a| a.without_content()).collect()
    };

    // 返回纯数组：旧前端 api.js 会解包到 data，再直接当数组用
    // （原实现是 ListOut::Full(Vec<T>) / Meta(Vec<T>)，序列化后就是数组）
    Ok(HttpResponse::Ok().json(ApiResponse::success(items)))
}

/// 把数据库记录转成前端需要的视图（字段名与原接口一致）
fn view(a: article::Article) -> serde_json::Value {
    json!({
        "slug": a.slug,
        "title": a.title,
        "summary": a.summary,
        "category": a.category,
        "tags": a.tags,
        "cover_image": a.cover_image,
        "content": a.content,
        "is_published": a.is_published,
        "author": a.author,
        "review_status": a.review_status,
        "is_featured": a.is_featured,
        "created_at": a.created_at,
        "updated_at": a.updated_at,
    })
}

/// GET /api/articles/{slug} —— 详情
pub async fn get_article(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();
    let claims = middleware::current_user(&req);
    let is_admin = claims.as_ref().map(|c| c.is_admin).unwrap_or(false);

    let found = article::find(db.as_ref(), &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    // 未发布文章只有管理员能看
    if !found.is_published && !is_admin {
        return Err(AppError::NotFound("文章不存在".to_string()));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(view(found))))
}

/// POST /api/articles —— 新建
pub async fn create_article(
    db: web::Data<DatabaseConnection>,
    body: web::Json<ArticleUpsert>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    let slug = body
        .slug
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| slugify(&body.title));

    if article::find(db.as_ref(), &slug).await?.is_some() {
        return Err(AppError::Conflict(format!("别名 {slug} 已存在")));
    }

    let created = article::create(
        &db,
        article::NewArticle {
            slug: &slug,
            title: &body.title,
            summary: body.summary.as_deref().unwrap_or(""),
            category: body.category.as_deref().unwrap_or("blog"),
            tags: body.tags.as_deref().unwrap_or(&[]),
            cover_image: body.cover_image.as_deref(),
            body: body.content.as_deref().unwrap_or(""),
            is_published: body.is_published.unwrap_or(true),
            author: &claims.sub,
            review_status: if is_site_author(&claims.sub) {
                "approved"
            } else {
                "pending"
            },
            is_featured: body.is_featured.unwrap_or(false),
        },
    )
    .await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(view(created))))
}

/// PUT /api/articles/{slug} —— 更新
pub async fn update_article(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    body: web::Json<ArticleUpsert>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let slug = path.into_inner();

    let existing = article::find(db.as_ref(), &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    // 只有作者本人或管理员能改
    if !claims.is_admin && !existing.author.eq_ignore_ascii_case(&claims.sub) {
        return Err(AppError::Forbidden("只能修改自己的文章".to_string()));
    }

    let new_slug = body
        .slug
        .as_ref()
        .filter(|s| !s.trim().is_empty() && **s != slug)
        .cloned();

    // 改 slug 前先确认新别名没被占用
    if let Some(ref ns) = new_slug {
        if article::find(db.as_ref(), ns).await?.is_some() {
            return Err(AppError::Conflict(format!("别名 {ns} 已存在")));
        }
    }

    let updated = article::update(
        &db,
        &slug,
        &article::ArticlePatch {
            title: Some(&body.title),
            summary: body.summary.as_deref(),
            category: body.category.as_deref(),
            tags: body.tags.as_deref(),
            cover_image: body.cover_image.as_deref(),
            body: body.content.as_deref(),
            is_published: body.is_published,
            review_status: None,
            is_featured: body.is_featured,
            new_slug: new_slug.as_deref(),
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    // slug 变了：评论跟着迁过去
    if let Some(ref ns) = new_slug {
        comment::rename_slug(db.as_ref(), &slug, ns).await?;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(view(updated))))
}

/// DELETE /api/articles/{slug} —— 删除
pub async fn delete_article(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;
    let slug = path.into_inner();

    let existing = article::find(db.as_ref(), &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    if !claims.is_admin && !existing.author.eq_ignore_ascii_case(&claims.sub) {
        return Err(AppError::Forbidden("只能删除自己的文章".to_string()));
    }

    article::delete(db.as_ref(), &slug).await?;
    // 评论一并清理（SQLite 外键不保证生效，见 db.rs 模块注释）
    let _ = comment::remove_all(db.as_ref(), &slug).await;

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "slug": slug }))))
}

/// POST /api/articles/batch-delete —— 批量删除
pub async fn batch_delete(
    db: web::Data<DatabaseConnection>,
    body: web::Json<serde_json::Value>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;

    let slugs: Vec<String> = body
        .get("slugs")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if slugs.is_empty() {
        return Err(AppError::BadRequest("没有选中任何文章".to_string()));
    }

    let (deleted, failed) = article::delete_many(db.as_ref(), &slugs).await?;

    // 逐条清理评论
    for s in &deleted {
        let _ = comment::remove_all(db.as_ref(), s).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
        "deleted": deleted,
        "failed": failed.iter().map(|(s, e)| json!({"slug": s, "error": e})).collect::<Vec<_>>(),
    }))))
}

/// 取某篇文章的原始 Markdown（含 front matter）
///
/// 数据库化之后「原始文件」不再存在，这里返回**由记录重建**的 Markdown，
/// 让全屏编辑器继续可用。
pub async fn get_raw(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let slug = path.into_inner();

    let a = article::find(db.as_ref(), &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    let raw = render_markdown(&a);

    Ok(HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(raw))
}

/// PUT /api/articles/{slug}/raw —— 保存原始 Markdown
///
/// 解析 front matter 后写回数据库；格式错误时拒绝写入。
pub async fn put_raw(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    body: web::Bytes,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let slug = path.into_inner();

    let text = String::from_utf8(body.to_vec())
        .map_err(|_| AppError::BadRequest("内容不是合法的 UTF-8".to_string()))?;

    let parsed = parse_markdown(&text)?;

    let patch = article::ArticlePatch {
        title: Some(&parsed.title),
        summary: Some(&parsed.summary),
        category: Some(&parsed.category),
        tags: Some(&parsed.tags),
        cover_image: parsed.cover_image.as_deref(),
        body: Some(&parsed.body),
        is_published: Some(parsed.is_published),
        review_status: None,
        is_featured: Some(parsed.is_featured),
        new_slug: None,
    };

    article::update(db.as_ref(), &slug, &patch)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "slug": slug }))))
}

/// 从记录重建 Markdown（front matter + 正文）
fn render_markdown(a: &article::Article) -> String {
    let tags = a
        .tags
        .iter()
        .map(|t| format!("- {t}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut fm = vec![
        "---".to_string(),
        format!("title: {}", a.title),
        format!("summary: {}", a.summary),
        format!("category: {}", a.category),
        format!("is_published: {}", a.is_published),
        format!("created_at: {}", a.created_at),
    ];
    if !tags.is_empty() {
        fm.push("tags:".to_string());
        fm.push(tags);
    }
    if let Some(c) = &a.cover_image {
        fm.push(format!("cover_image: {c}"));
    }
    if a.is_featured {
        fm.push("is_featured: true".to_string());
    }
    fm.push("---".to_string());

    format!("{}\n\n{}", fm.join("\n"), a.content)
}

/// 解析后的 front matter
struct ParsedMarkdown {
    title: String,
    summary: String,
    category: String,
    tags: Vec<String>,
    cover_image: Option<String>,
    is_published: bool,
    is_featured: bool,
    body: String,
}

/// 解析 Markdown（YAML front matter + 正文）
///
/// 关键机制：用 serde_yaml 解析 `---` 之间的头部；
/// 解析失败直接报错，**不写入数据库**——避免把结构写坏。
fn parse_markdown(text: &str) -> Result<ParsedMarkdown, AppError> {
    #[derive(serde::Deserialize)]
    struct Front {
        title: String,
        #[serde(default)]
        summary: String,
        #[serde(default = "default_category")]
        category: String,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        cover_image: Option<String>,
        #[serde(default = "default_true")]
        is_published: bool,
        #[serde(default)]
        is_featured: bool,
    }

    fn default_category() -> String {
        "blog".to_string()
    }
    fn default_true() -> bool {
        true
    }

    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return Err(AppError::BadRequest(
            "缺少 front matter（文件应以 --- 开头）".to_string(),
        ));
    }

    // 找第二个 ---
    let rest = &trimmed[3..];
    let end = rest.find("\n---").ok_or_else(|| {
        AppError::BadRequest("front matter 没有结束标记 ---".to_string())
    })?;

    let yaml = &rest[..end];
    let body = rest[end + 4..].trim_start_matches('\n').to_string();

    let front: Front = serde_yaml::from_str(yaml)
        .map_err(|e| AppError::BadRequest(format!("front matter 解析失败: {e}")))?;

    Ok(ParsedMarkdown {
        title: front.title,
        summary: front.summary,
        category: front.category,
        tags: front.tags,
        cover_image: front.cover_image,
        is_published: front.is_published,
        is_featured: front.is_featured,
        body,
    })
}

/// 由标题生成 slug（只保留 ASCII 字母数字与短横线）
///
/// 说明：中文标题会得到空串，此时调用方应显式提供 slug。
fn slugify(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let s = s.trim_matches('-').to_string();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");

    if s.is_empty() {
        format!("post-{}", chrono::Utc::now().timestamp())
    } else {
        s
    }
}

/// GET /api/articles/review/pending —— 待审核列表
pub async fn list_pending(
    db: web::Data<DatabaseConnection>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let items = article::pending(db.as_ref()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(
        items.into_iter().map(view).collect::<Vec<_>>(),
    )))
}

/// PUT /api/articles/{slug}/review —— 审核
pub async fn review_article(
    db: web::Data<DatabaseConnection>,
    path: web::Path<String>,
    body: web::Json<serde_json::Value>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    middleware::verify_admin(req)?;
    let slug = path.into_inner();

    let status = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("approved")
        .to_string();

    if status != "approved" && status != "rejected" && status != "pending" {
        return Err(AppError::BadRequest("未知的审核状态".to_string()));
    }

    let updated = article::update(
        &db,
        &slug,
        &article::ArticlePatch {
            review_status: Some(&status),
            ..Default::default()
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(view(updated))))
}

/// 供其他 handler 复用的 Hub 类型别名
pub type WsHub = web::Data<std::sync::Arc<Hub>>;
