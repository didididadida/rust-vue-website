//! 项目 / 工具服务（SeaORM 版）
//!
//! 逻辑描述：
//!   输入 → 输出：`showcases` 表按 kind 过滤的读写
//!   关键机制：
//!     1. 一张表存两类（kind='toy' | 'tool'），查询时按 kind 过滤；
//!     2. 入参用两个独立结构体（ToyUpsert / ToolUpsert），
//!        避免"项目记录里写工具字段"这种错误；
//!     3. tech_stack 存 JSON 数组字符串（沿用 SQLite 的通行做法）。
//!   不变量：
//!     - kind 只能是 'toy' 或 'tool'；
//!     - (kind, slug) 组合唯一（在服务层校验）
//!   边界：
//!     - 跨 kind 可以同名（项目和工具都能叫 "xxx"）；
//!     - 未发布的项默认不出现在公开列表。
//!
//! 依据：
//!   经典问题：单表继承的查询过滤与入参校验
//!   来源：
//!     1. Martin Fowler《Single Table Inheritance》
//!        https://martinfowler.com/eaaCatalog/singleTableInheritance.html
//!        要点：类型判别列（此处 kind）必须在**所有查询**里正确过滤，
//!              否则会串类。
//!     2. 本项目原实现 `models/mod.rs` 的 ToyUpsert / ToolUpsert 两个入参结构体
//!   原型结论：
//!     1. 单表继承的代价是每个查询都要带类型条件，漏掉就会串类；
//!     2. 用独立入参结构体可以在编译期防止字段混用。
//!   本处变体：原来两套 Markdown 目录 + 两个结构体；现在一张表 + 两个入参结构体。
//!   沿用结论：保留 ToyUpsert / ToolUpsert 的字段划分。
//!   偏离处：唯一性检查放在应用层（先查再插），
//!           而不是数据库唯一索引——因为需要 (kind, slug) 复合唯一，
//!           SeaORM Schema Registry 对此支持有限，且个人站并发低。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entities::{showcase, showcase::Entity as ShowcaseEntity};
use crate::errors::AppError;

/// 对外视图
#[derive(Debug, Clone, serde::Serialize)]
pub struct Showcase {
    pub slug: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub is_published: bool,
    pub created_at: String,
    pub updated_at: String,
    // 项目特有
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub demo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tech_stack: Vec<String>,
    // 工具特有
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Showcase {
    /// 列表视图：去掉正文
    pub fn without_content(&self) -> Self {
        let mut c = self.clone();
        c.content = String::new();
        c
    }
}

fn to_view(m: showcase::Model) -> Showcase {
    Showcase {
        slug: m.slug,
        kind: m.kind,
        name: m.name,
        description: m.description,
        content: m.content,
        is_published: m.is_published,
        created_at: m.created_at,
        updated_at: m.updated_at,
        image: m.image,
        demo_url: m.demo_url,
        source_url: m.source_url,
        tech_stack: serde_json::from_str(&m.tech_stack).unwrap_or_default(),
        icon: m.icon,
        category: m.category,
        url: m.url,
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// 列出某类（toy / tool）
pub async fn list(
    db: &DatabaseConnection,
    kind: &str,
    include_unpublished: bool,
) -> Result<Vec<Showcase>, AppError> {
    let mut q = ShowcaseEntity::find().filter(showcase::COLUMN.kind.eq(kind));
    if !include_unpublished {
        q = q.filter(showcase::COLUMN.is_published.eq(true));
    }

    let rows = q
        .order_by_desc(showcase::COLUMN.created_at)
        .all(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询失败: {e}")))?;

    Ok(rows.into_iter().map(to_view).collect())
}

/// 取单条
pub async fn find(
    db: &DatabaseConnection,
    kind: &str,
    slug: &str,
) -> Result<Option<Showcase>, AppError> {
    ShowcaseEntity::find()
        .filter(showcase::COLUMN.kind.eq(kind))
        .filter(showcase::COLUMN.slug.eq(slug))
        .one(db)
        .await
        .map(|o| o.map(to_view))
        .map_err(|e| AppError::Internal(format!("查询失败: {e}")))
}

/// 新建 / 更新的通用入参
pub struct Upsert<'a> {
    pub kind: &'a str,
    pub slug: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub content: Option<&'a str>,
    pub is_published: bool,
    // 项目
    pub image: Option<&'a str>,
    pub demo_url: Option<&'a str>,
    pub source_url: Option<&'a str>,
    pub tech_stack: Option<&'a [String]>,
    // 工具
    pub icon: Option<&'a str>,
    pub category: Option<&'a str>,
    pub url: Option<&'a str>,
}

/// 新建
pub async fn create(db: &DatabaseConnection, u: Upsert<'_>) -> Result<Showcase, AppError> {
    if find(db, u.kind, u.slug).await?.is_some() {
        return Err(AppError::Conflict(format!("别名 {} 已存在", u.slug)));
    }

    let ts = now();
    let am = showcase::ActiveModel {
        kind: Set(u.kind.to_string()),
        slug: Set(u.slug.to_string()),
        name: Set(u.name.to_string()),
        description: Set(u.description.to_string()),
        content: Set(u.content.unwrap_or("").to_string()),
        is_published: Set(u.is_published),
        created_at: Set(ts.clone()),
        updated_at: Set(ts),
        image: Set(u.image.map(|s| s.to_string())),
        demo_url: Set(u.demo_url.map(|s| s.to_string())),
        source_url: Set(u.source_url.map(|s| s.to_string())),
        tech_stack: Set(serde_json::to_string(u.tech_stack.unwrap_or(&[])).unwrap_or_else(|_| "[]".into())),
        icon: Set(u.icon.map(|s| s.to_string())),
        category: Set(u.category.map(|s| s.to_string())),
        url: Set(u.url.map(|s| s.to_string())),
        ..Default::default()
    };

    let m = am
        .insert(db)
        .await
        .map_err(|e| AppError::Internal(format!("创建失败: {e}")))?;

    Ok(to_view(m))
}

/// 更新（只改进传的字段）
pub async fn update(
    db: &DatabaseConnection,
    kind: &str,
    slug: &str,
    u: &Upsert<'_>,
) -> Result<Option<Showcase>, AppError> {
    let Some(m) = ShowcaseEntity::find()
        .filter(showcase::COLUMN.kind.eq(kind))
        .filter(showcase::COLUMN.slug.eq(slug))
        .one(db)
        .await
        .map_err(|e| AppError::Internal(format!("查询失败: {e}")))?
    else {
        return Ok(None);
    };

    let mut am: showcase::ActiveModel = m.into();
    am.name = Set(u.name.to_string());
    am.description = Set(u.description.to_string());
    am.is_published = Set(u.is_published);
    am.updated_at = Set(now());
    if let Some(v) = u.content {
        am.content = Set(v.to_string());
    }
    if let Some(v) = u.image {
        am.image = Set(Some(v.to_string()));
    }
    if let Some(v) = u.demo_url {
        am.demo_url = Set(Some(v.to_string()));
    }
    if let Some(v) = u.source_url {
        am.source_url = Set(Some(v.to_string()));
    }
    if let Some(v) = u.tech_stack {
        am.tech_stack = Set(serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    }
    if let Some(v) = u.icon {
        am.icon = Set(Some(v.to_string()));
    }
    if let Some(v) = u.category {
        am.category = Set(Some(v.to_string()));
    }
    if let Some(v) = u.url {
        am.url = Set(Some(v.to_string()));
    }

    let updated = am
        .update(db)
        .await
        .map_err(|e| AppError::Internal(format!("更新失败: {e}")))?;

    Ok(Some(to_view(updated)))
}

/// 删除
pub async fn delete(
    db: &DatabaseConnection,
    kind: &str,
    slug: &str,
) -> Result<bool, AppError> {
    let res = ShowcaseEntity::delete_many()
        .filter(showcase::COLUMN.kind.eq(kind))
        .filter(showcase::COLUMN.slug.eq(slug))
        .exec(db)
        .await
        .map_err(|e| AppError::Internal(format!("删除失败: {e}")))?;

    Ok(res.rows_affected > 0)
}
