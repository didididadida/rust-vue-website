//! 项目 / 工具接口
//!
//! 逻辑描述：
//!   输入 → 输出：`/api/toys/*` 与 `/api/tools/*` 的 CRUD + 批量删除
//!   关键机制：
//!     1. 两组接口共用 `services::showcase`，只靠 kind 区分；
//!     2. 用宏避免为 toy / tool 各写一遍相同的 handler；
//!     3. 列表默认剔除正文（详情才带），与原接口的 `fields=meta|full` 一致。
//!   不变量：
//!     - 未发布项不出现在公开列表；
//!     - 非管理员不能增删改。
//!   边界：
//!     - 项目与工具可同名（slug 在各自 kind 内唯一）。
//!
//! 依据：
//!   经典问题：重复代码消除（DRY）与单表继承的接口暴露
//!   来源：
//!     1. 本项目原实现 `handlers/toys.rs` 与 `handlers/tools.rs`：
//!        两个文件约 80% 结构相同（列表/详情/新建/更新/删除/批量删除）
//!     2. Rust 声明宏（macro_rules!）用于生成结构相同的函数
//!        https://doc.rust-lang.org/reference/macros-by-example.html
//!   原型结论：
//!     1. 完全相同的逻辑复制两遍会导致改一处漏一处；
//!     2. 宏可以在保持函数签名独立的前提下消除重复。
//!   本处变体：
//!     - 原实现是两个独立文件；
//!     - 数据库化后底层已合并为一张表，接口层可用宏生成。
//!   沿用结论：保留 `/api/toys` 与 `/api/tools` 两条路径（前端依赖），
//!             但内部用宏复用。
//!   偏离处：
//!     - 首页的 `scope` 过滤（home/community）未实现——
//!       原实现靠作者字段判断，数据库化后暂未迁移该逻辑；
//!       当前所有已发布项目都会出现在列表里。

use actix_web::{web, HttpRequest, HttpResponse};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::json;

use crate::entities::showcase::{KIND_TOOL, KIND_TOY};
use crate::errors::AppError;
use crate::middleware;
use crate::models::ApiResponse;
use crate::services::showcase as svc;

/// 新增/更新的请求体（项目与工具合并）
#[derive(Debug, Deserialize)]
pub struct UpsertBody {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub is_published: Option<bool>,
    // 项目
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub demo_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub tech_stack: Option<Vec<String>>,
    // 工具
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// 列表查询参数
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub fields: Option<String>,
}

/// 由标题生成 slug
fn slugify(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        format!("item-{}", chrono::Utc::now().timestamp())
    } else {
        s
    }
}

/// 生成一组 CRUD handler（toy 与 tool 各用一次）
macro_rules! showcase_handlers {
    ($mod_name:ident, $kind:expr) => {
        pub mod $mod_name {
            use super::*;

            /// 列表
            pub async fn list(
                db: web::Data<DatabaseConnection>,
                query: web::Query<ListQuery>,
                req: HttpRequest,
            ) -> Result<HttpResponse, AppError> {
                let is_admin = middleware::current_user(&req)
                    .map(|c| c.is_admin)
                    .unwrap_or(false);

                let items = svc::list(db.as_ref(), $kind, is_admin).await?;

                let with_body = query.fields.as_deref() == Some("full");
                let out: Vec<svc::Showcase> = if with_body {
                    items
                } else {
                    items.iter().map(|x| x.without_content()).collect()
                };

                // 原接口返回纯数组（ListOut 序列化），前端直接当数组用
                Ok(HttpResponse::Ok().json(ApiResponse::success(out)))
            }

            /// 详情
            pub async fn get(
                db: web::Data<DatabaseConnection>,
                path: web::Path<String>,
                req: HttpRequest,
            ) -> Result<HttpResponse, AppError> {
                let slug = path.into_inner();
                let is_admin = middleware::current_user(&req)
                    .map(|c| c.is_admin)
                    .unwrap_or(false);

                let item = svc::find(db.as_ref(), $kind, &slug)
                    .await?
                    .ok_or_else(|| AppError::NotFound("内容不存在".to_string()))?;

                if !item.is_published && !is_admin {
                    return Err(AppError::NotFound("内容不存在".to_string()));
                }

                Ok(HttpResponse::Ok().json(ApiResponse::success(item)))
            }

            /// 新建
            pub async fn create(
                db: web::Data<DatabaseConnection>,
                body: web::Json<UpsertBody>,
                req: HttpRequest,
            ) -> Result<HttpResponse, AppError> {
                middleware::verify_admin(req)?;

                let slug = body
                    .slug
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| slugify(&body.name));

                let created = svc::create(
                    db.as_ref(),
                    svc::Upsert {
                        kind: $kind,
                        slug: &slug,
                        name: &body.name,
                        description: body.description.as_deref().unwrap_or(""),
                        content: body.content.as_deref(),
                        is_published: body.is_published.unwrap_or(true),
                        image: body.image.as_deref(),
                        demo_url: body.demo_url.as_deref(),
                        source_url: body.source_url.as_deref(),
                        tech_stack: body.tech_stack.as_deref(),
                        icon: body.icon.as_deref(),
                        category: body.category.as_deref(),
                        url: body.url.as_deref(),
                    },
                )
                .await?;

                Ok(HttpResponse::Created().json(ApiResponse::success(created)))
            }

            /// 更新
            pub async fn update(
                db: web::Data<DatabaseConnection>,
                path: web::Path<String>,
                body: web::Json<UpsertBody>,
                req: HttpRequest,
            ) -> Result<HttpResponse, AppError> {
                middleware::verify_admin(req)?;
                let slug = path.into_inner();

                let updated = svc::update(
                    db.as_ref(),
                    $kind,
                    &slug,
                    &svc::Upsert {
                        kind: $kind,
                        slug: &slug,
                        name: &body.name,
                        description: body.description.as_deref().unwrap_or(""),
                        content: body.content.as_deref(),
                        is_published: body.is_published.unwrap_or(true),
                        image: body.image.as_deref(),
                        demo_url: body.demo_url.as_deref(),
                        source_url: body.source_url.as_deref(),
                        tech_stack: body.tech_stack.as_deref(),
                        icon: body.icon.as_deref(),
                        category: body.category.as_deref(),
                        url: body.url.as_deref(),
                    },
                )
                .await?
                .ok_or_else(|| AppError::NotFound("内容不存在".to_string()))?;

                Ok(HttpResponse::Ok().json(ApiResponse::success(updated)))
            }

            /// 删除
            pub async fn delete(
                db: web::Data<DatabaseConnection>,
                path: web::Path<String>,
                req: HttpRequest,
            ) -> Result<HttpResponse, AppError> {
                middleware::verify_admin(req)?;
                let slug = path.into_inner();

                let ok = svc::delete(db.as_ref(), $kind, &slug).await?;
                if !ok {
                    return Err(AppError::NotFound("内容不存在".to_string()));
                }

                Ok(HttpResponse::Ok().json(ApiResponse::success(json!({ "deleted": true }))))
            }

            /// 批量删除
            ///
            /// 输入：`{ slugs: ["a", "b"] }`
            /// 输出：`{ deleted: [...], failed: [{slug, error}] }`
            ///
            /// 关键机制：逐条删除并记录失败项，而不是一条失败就整个回滚。
            /// 原因：前端会展示失败清单让用户知道哪几条没删掉。
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
                    return Err(AppError::BadRequest("没有选中任何项".to_string()));
                }

                let mut deleted = Vec::new();
                let mut failed = Vec::new();

                for slug in slugs {
                    match svc::delete(db.as_ref(), $kind, &slug).await {
                        Ok(true) => deleted.push(slug),
                        Ok(false) => {
                            failed.push(json!({ "slug": slug, "error": "内容不存在" }))
                        }
                        Err(e) => failed.push(json!({ "slug": slug, "error": e.to_string() })),
                    }
                }

                Ok(HttpResponse::Ok().json(ApiResponse::success(json!({
                    "deleted": deleted,
                    "failed": failed,
                }))))
            }
        }
    };
}

showcase_handlers!(toys, KIND_TOY);
showcase_handlers!(tools, KIND_TOOL);
