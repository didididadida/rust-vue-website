//! 项目 / 工具迁移：Markdown 文件 → showcases 表
//!
//! 逻辑描述：
//!   输入 → 输出：
//!     data/toys/*.md   → showcases 表（kind='toy'）
//!     data/tools/*.md  → showcases 表（kind='tool'）
//!   关键机制：
//!     1. 解析 front matter 得到字段，正文作为 content；
//!     2. 按 (kind, slug) 判重，已存在则跳过；
//!     3. tech_stack 在 front matter 里可能是数组或字符串，两种都接受。
//!   不变量：
//!     - 源文件不删除（保留作为回滚依据）
//!     - 幂等：重跑不会重复插入
//!   边界：
//!     - 目录不存在 → 跳过，不报错
//!     - front matter 解析失败 → 记为该条失败，继续下一条
//!
//! 依据：
//!   经典问题：异构字段的解析容忍（lenient parsing）
//!   来源：
//!     1. 本项目原 `content/mod.rs` 的 ToyRecord / ToolRecord，
//!        以及 `de_string_list` 自定义反序列化（兼容
//!        YAML 里 tags/tech_stack 写成数组或逗号字符串两种形式）
//!     2. 本机全局约定「脚本与代码规范」第 2 节：脚本必须幂等
//!   原型结论：
//!     1. 手写的 YAML 里同一字段可能是 `[a, b]` 或 `a, b`，
//!        解析要能容忍两种；
//!     2. 迁移脚本重跑不能改变结果。
//!   本处变体：原实现从两个目录分别读；现在合并进一张表用 kind 区分。
//!   沿用结论：保留 de_string_list 的容忍行为；按 (kind, slug) 判重。
//!   偏离处：
//!     - 不保留原文件的"目录即类型"信息（改由 kind 字段承载）；
//!     - 未迁移 original 里的 figure/ 等附件目录（它们属于文章内容）。

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter};
use serde::Deserialize;

use crate::entities::showcase::{Entity as ShowcaseEntity, KIND_TOOL, KIND_TOY};
use crate::entities::showcase;
use crate::errors::AppError;
use crate::services::showcase as svc;

/// 项目 / 工具的 front matter
#[derive(Debug, Deserialize)]
struct Front {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    is_published: Option<bool>,
    // 项目
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    demo_url: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default, deserialize_with = "de_string_list")]
    tech_stack: Vec<String>,
    // 工具
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

/// 容忍 tech_stack 写成数组或逗号分隔字符串
fn de_string_list<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    match Option::<OneOrMany>::deserialize(d)? {
        None => Ok(vec![]),
        Some(OneOrMany::One(s)) => Ok(s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()),
        Some(OneOrMany::Many(v)) => Ok(v),
    }
}

/// 迁移统计
#[derive(Debug, Default)]
pub struct ShowcaseReport {
    pub toys: usize,
    pub tools: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

impl ShowcaseReport {
    pub fn summary(&self) -> String {
        format!(
            "项目 {} / 工具 {}（跳过 {}，错误 {}）",
            self.toys,
            self.tools,
            self.skipped,
            self.errors.len()
        )
    }
}

/// 执行迁移
pub async fn run(db: &DatabaseConnection, dry_run: bool) -> Result<ShowcaseReport, AppError> {
    let mut report = ShowcaseReport::default();
    let root = crate::paths::root();

    // 两组合并：(目录, kind, 计数器)
    for (subdir, kind) in [("toys", KIND_TOY), ("tools", KIND_TOOL)] {
        let dir = root.join("data").join(subdir);
        if !dir.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|x| x.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
            })
            .collect();
        files.sort_by_key(|e| e.file_name());

        for entry in files {
            let path = entry.path();
            let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let slug = slug.to_string();

            // 判重
            let exists = ShowcaseEntity::find()
                .filter(showcase::COLUMN.kind.eq(kind))
                .filter(showcase::COLUMN.slug.eq(slug.clone()))
                .one(db)
                .await
                .map_err(|e| AppError::Internal(format!("查重失败: {e}")))?;

            if exists.is_some() {
                report.skipped += 1;
                continue;
            }

            match migrate_one(db, kind, &slug, &path, dry_run).await {
                Ok(()) => {
                    if kind == KIND_TOY {
                        report.toys += 1;
                    } else {
                        report.tools += 1;
                    }
                }
                Err(e) => report.errors.push(format!("{subdir}/{slug}: {e}")),
            }
        }
    }

    Ok(report)
}

async fn migrate_one(
    db: &DatabaseConnection,
    kind: &str,
    slug: &str,
    path: &std::path::Path,
    dry_run: bool,
) -> Result<(), AppError> {
    let text = std::fs::read_to_string(path)?;
    let (front_yaml, body) = split_front_matter(&text)?;

    let f: Front = serde_yaml::from_str(&front_yaml)
        .map_err(|e| AppError::BadRequest(format!("front matter 解析失败: {e}")))?;

    if dry_run {
        return Ok(());
    }

    svc::create(
        db,
        svc::Upsert {
            kind,
            slug,
            name: &f.name,
            description: &f.description,
            content: Some(&body),
            is_published: f.is_published.unwrap_or(true),
            image: f.image.as_deref(),
            demo_url: f.demo_url.as_deref(),
            source_url: f.source_url.as_deref(),
            tech_stack: Some(&f.tech_stack),
            icon: f.icon.as_deref(),
            category: f.category.as_deref(),
            url: f.url.as_deref(),
        },
    )
    .await?;

    Ok(())
}

/// 拆分 front matter 与正文
fn split_front_matter(text: &str) -> Result<(String, String), AppError> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return Err(AppError::BadRequest("缺少 front matter".to_string()));
    }

    let rest = &trimmed[3..];
    let end = rest
        .find("\n---")
        .ok_or_else(|| AppError::BadRequest("front matter 未闭合".to_string()))?;

    let yaml = rest[..end].to_string();
    let body = rest[end + 4..].trim_start_matches('\n').to_string();

    Ok((yaml, body))
}
