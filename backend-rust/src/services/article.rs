//! 文章服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：`articles` 表的读写 —— 列表、详情、新建、更新、删除、审核
//!   关键机制：
//!     1. `slug` 是唯一对外标识，`id` 是内部主键；
//!     2. 标签以 JSON 数组字符串存（SQLite 无原生数组），读写时转换；
//!     3. 列表默认剔除正文（只返回元数据），详情才带全文——
//!        正文可能很长，列表页不需要。
//!   不变量：
//!     - slug 唯一；
//!     - `created_at` 只在首次创建时写入，更新不覆盖。
//!   边界：
//!     - 删除文章时其评论由调用方（handlers）一并清理；
//!     - 未发布文章（is_published=false）默认不出现在公开列表。
//!
//! 依据：
//!   经典问题：列表与详情的字段裁剪（projection / 按需加载）
//!   来源：
//!     1. 本项目原实现 `models/mod.rs` 的 `ArticleView::without_content()`，
//!        其注释："输入 → 输出：持有完整正文的 ArticleView → content 置空、
//!        其余字段原样复制"，以及 `ListOut<T>` 的 `fields=meta|full` 参数。
//!     2. SQLite 官方文档：无原生数组类型，建议存 JSON 文本
//!        https://sqlite.org/datatype3.html
//!   原型结论：
//!     1. 列表页剔除正文可显著减少传输量（正文常有几十 KB）；
//!     2. 多值字段在 SQLite 里存 JSON 字符串是标准做法。
//!   本处变体：从 Markdown 文件迁到表后，正文成为一列，
//!             更需要"列表不返回正文"这个裁剪。
//!   沿用结论：保留 meta/full 两种视图；标签存 JSON 字符串。
//!   偏离处：暂时没有做全文搜索（原实现也没有）。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};

use crate::entities::{article, article::Entity as ArticleEntity};
use crate::errors::AppError;

/// 对外返回的文章视图
#[derive(Debug, Clone, serde::Serialize)]
pub struct Article {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub category: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<String>,
    /// 列表视图时为空字符串
    pub content: String,
    pub is_published: bool,
    pub author: String,
    pub review_status: String,
    pub is_featured: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Article {
    /// 去掉正文（列表视图）
    ///
    /// 沿用原 `ArticleView::without_content` 的语义：
    /// 除 content 外所有字段原样保留，调用方拿到的仍是同一条记录。
    pub fn without_content(&self) -> Self {
        let mut c = self.clone();
        c.content = String::new();
        c
    }
}

/// 把数据库行转成对外视图
fn to_view(m: article::Model) -> Article {
    Article {
        slug: m.slug,
        title: m.title,
        summary: m.summary,
        category: m.category,
        tags: parse_tags(&m.tags),
        cover_image: m.cover_image,
        content: m.body,
        is_published: m.is_published,
        author: m.author,
        review_status: m.review_status,
        is_featured: m.is_featured,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

/// 解析标签 JSON；解析失败时退化为空（不让脏数据导致整个列表报错）
fn parse_tags(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

/// 序列化标签
fn dump_tags(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

/// 列表查询参数
#[derive(Debug, Default, Clone)]
pub struct ListParams {
    pub category: Option<String>,
    /// 是否包含未发布文章（管理员视图）
    pub include_unpublished: bool,
    /// 只看某个作者
    pub author: Option<String>,
    pub skip: u64,
    pub limit: u64,
}

/// 列表查询结果
pub struct ListResult {
    pub total: u64,
    pub items: Vec<Article>,
}

/// 查询文章列表
pub async fn list(db: &DatabaseConnection, p: &ListParams) -> Result<ListResult, AppError> {
    let mut q = ArticleEntity::find();

    if let Some(cat) = &p.category {
        if !cat.is_empty() && cat != "all" {
            q = q.filter(article::COLUMN.category.eq(cat.clone()));
        }
    }
    if !p.include_unpublished {
        q = q.filter(article::COLUMN.is_published.eq(true));
    }
    if let Some(author) = &p.author {
        q = q.filter(article::COLUMN.author.eq(author.clone()));
    }

    let total = q
        .clone()
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计文章失败: {e}")))?;

    let rows = q
        .order_by_desc(article::COLUMN.created_at)
        .offset(p.skip)
        .limit(p.limit)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询文章失败: {e}")))?;

    Ok(ListResult {
        total,
        items: rows.into_iter().map(to_view).collect(),
    })
}

/// 按 slug 取单篇
pub async fn find(db: &DatabaseConnection, slug: &str) -> Result<Option<Article>, AppError> {
    ArticleEntity::find()
        .filter(article::COLUMN.slug.eq(slug))
        .one(db)
        .await
        .map(|opt| opt.map(to_view))
        .map_err(|e| AppError::Internal(format!("查询文章失败: {e}")))
}

/// 新建文章入参
pub struct NewArticle<'a> {
    pub slug: &'a str,
    pub title: &'a str,
    pub summary: &'a str,
    pub category: &'a str,
    pub tags: &'a [String],
    pub cover_image: Option<&'a str>,
    pub body: &'a str,
    pub is_published: bool,
    pub author: &'a str,
    pub review_status: &'a str,
    pub is_featured: bool,
}

/// 新建文章
pub async fn create(db: &DatabaseConnection, a: NewArticle<'_>) -> Result<Article, AppError> {
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let am = article::ActiveModel {
        slug: Set(a.slug.to_string()),
        title: Set(a.title.to_string()),
        summary: Set(a.summary.to_string()),
        category: Set(a.category.to_string()),
        tags: Set(dump_tags(a.tags)),
        cover_image: Set(a.cover_image.map(|s| s.to_string())),
        body: Set(a.body.to_string()),
        is_published: Set(a.is_published),
        author: Set(a.author.to_string()),
        review_status: Set(a.review_status.to_string()),
        is_featured: Set(a.is_featured),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        ..Default::default()
    };

    let model = am
        .insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("创建文章失败: {e}")))?;

    Ok(to_view(model))
}

/// 更新文章入参（全部可选，None 表示不改）
#[derive(Default)]
pub struct ArticlePatch<'a> {
    pub title: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub category: Option<&'a str>,
    pub tags: Option<&'a [String]>,
    pub cover_image: Option<&'a str>,
    pub body: Option<&'a str>,
    pub is_published: Option<bool>,
    pub review_status: Option<&'a str>,
    pub is_featured: Option<bool>,
    /// 改 slug（伴随重命名）
    pub new_slug: Option<&'a str>,
}

/// 更新文章
pub async fn update(
    db: &DatabaseConnection,
    slug: &str,
    p: &ArticlePatch<'_>,
) -> Result<Option<Article>, AppError> {
    let Some(model) = ArticleEntity::find()
        .filter(article::COLUMN.slug.eq(slug))
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询文章失败: {e}")))?
    else {
        return Ok(None);
    };

    let mut am: article::ActiveModel = model.into();

    if let Some(v) = p.title {
        am.title = Set(v.to_string());
    }
    if let Some(v) = p.summary {
        am.summary = Set(v.to_string());
    }
    if let Some(v) = p.category {
        am.category = Set(v.to_string());
    }
    if let Some(v) = p.tags {
        am.tags = Set(dump_tags(v));
    }
    if let Some(v) = p.cover_image {
        am.cover_image = Set(Some(v.to_string()));
    }
    if let Some(v) = p.body {
        am.body = Set(v.to_string());
    }
    if let Some(v) = p.is_published {
        am.is_published = Set(v);
    }
    if let Some(v) = p.review_status {
        am.review_status = Set(v.to_string());
    }
    if let Some(v) = p.is_featured {
        am.is_featured = Set(v);
    }
    if let Some(v) = p.new_slug {
        am.slug = Set(v.to_string());
    }

    am.updated_at = Set(Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));

    let updated = am
        .update(db)
        .await
        .map_err(|e| AppError::Internal(format!("更新文章失败: {e}")))?;

    Ok(Some(to_view(updated)))
}

/// 删除文章；返回是否真的删掉了
///
/// 注意：调用方还需要清理该文章的评论（见 `services::comment::remove_all`）。
pub async fn delete(db: &DatabaseConnection, slug: &str) -> Result<bool, AppError> {
    let res = ArticleEntity::delete_many()
        .filter(article::COLUMN.slug.eq(slug))
        .exec(db)
        .await
        .map_err(|e| AppError::Internal(format!("删除文章失败: {e}")))?;

    Ok(res.rows_affected > 0)
}

/// 批量删除；返回成功与失败的 slug
pub async fn delete_many(
    db: &DatabaseConnection,
    slugs: &[String],
) -> Result<(Vec<String>, Vec<(String, String)>), AppError> {
    let mut ok = Vec::new();
    let mut failed = Vec::new();

    for slug in slugs {
        match delete(db, slug).await {
            Ok(true) => ok.push(slug.clone()),
            Ok(false) => failed.push((slug.clone(), "文章不存在".to_string())),
            Err(e) => failed.push((slug.clone(), e.to_string())),
        }
    }

    Ok((ok, failed))
}

/// 待审核列表
pub async fn pending(db: &DatabaseConnection) -> Result<Vec<Article>, AppError> {
    let rows = ArticleEntity::find()
        .filter(article::COLUMN.review_status.eq("pending"))
        .order_by_asc(article::COLUMN.created_at)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询待审核失败: {e}")))?;

    Ok(rows.into_iter().map(to_view).collect())
}

/// 文章总数（含未发布的，后台统计用）
pub async fn count_all(db: &DatabaseConnection) -> Result<u64, AppError> {
    ArticleEntity::find()
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计文章失败: {e}")))
}
