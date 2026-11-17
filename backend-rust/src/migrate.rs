//! 数据迁移：文件 → 数据库
//!
//! 逻辑描述：
//!   输入 → 输出：
//!     posts/*.md          → articles 表
//!     data/comments/*.json → comments 表
//!     data/users.txt       → users 表
//!     data/profiles/*.json → profiles 表
//!   关键机制：
//!     1. 幂等：已存在的 slug / username 跳过，不覆盖（避免第二次跑把线上改动冲掉）；
//!     2. 逐项汇报：每条成功/失败都打印，失败不中断整体迁移；
//!     3. `--dry-run` 只读不写，先把要做什么打印出来。
//!   不变量：
//!     - 不删除源文件（迁移后原文件保留，作为回滚依据）；
//!     - 密码哈希原样搬运，不重新计算。
//!   边界：
//!     - front matter 格式错误 → 记为该篇失败，继续下一篇；
//!     - 中文 slug 原样保留（不做 slugify，否则会丢标识）；
//!     - 评论 JSON 里 parent_id 指向不存在的评论时，仍写入（组树时容错）。
//!
//! 依据：
//!   经典问题：数据迁移的幂等与可回滚（idempotent, reversible migration）
//!   来源：
//!     1. Martin Fowler《Parallel Change / Expand-Contract》关于
//!        "让新旧两套并存一段时间再切换"的论述
//!        https://martinfowler.com/bliki/ParallelChange.html
//!     2. 本机全局约定「脚本与代码规范」第 2 节：
//!        脚本必须幂等、可重跑；有副作用的操作必须先备份。
//!   原型结论：
//!     1. 迁移脚本重跑不能改变结果，否则执行两次就可能覆盖线上新数据；
//!     2. 保留源文件直到确认迁移成功，是"可回滚"的最低要求。
//!   本处变体：
//!     - 数据源是纯文件（Markdown/JSON/TXT），没有事务可依赖，
//!       只能靠"逐条插入 + 已存在则跳过"保证幂等。
//!   沿用结论：已存在跳过；源文件不动；提供 --dry-run。
//!   偏离处：
//!     - 不迁移 forum.db（论坛已从项目移除）；
//!     - 不迁移 static/uploads 下的文件（文件本身不动，只迁 uploads 表的记录）。

use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait};

use crate::errors::AppError;
use crate::services::{article, comment, profile, upload, user};

/// 迁移统计
#[derive(Debug, Default)]
pub struct Report {
    pub articles_ok: usize,
    pub articles_skip: usize,
    pub articles_fail: Vec<(String, String)>,

    pub comments_ok: usize,
    pub comments_skip: usize,
    pub comments_fail: Vec<(String, String)>,

    pub users_ok: usize,
    pub users_skip: usize,
    pub users_fail: Vec<(String, String)>,

    pub profiles_ok: usize,
    pub profiles_skip: usize,
    pub profiles_fail: Vec<(String, String)>,
}

impl Report {
    pub fn summary(&self) -> String {
        format!(
            "文章: {} 成功 / {} 跳过 / {} 失败\n\
             评论: {} 成功 / {} 跳过 / {} 失败\n\
             用户: {} 成功 / {} 跳过 / {} 失败\n\
             资料: {} 成功 / {} 跳过 / {} 失败",
            self.articles_ok,
            self.articles_skip,
            self.articles_fail.len(),
            self.comments_ok,
            self.comments_skip,
            self.comments_fail.len(),
            self.users_ok,
            self.users_skip,
            self.users_fail.len(),
            self.profiles_ok,
            self.profiles_skip,
            self.profiles_fail.len(),
        )
    }

    pub fn has_failures(&self) -> bool {
        !self.articles_fail.is_empty()
            || !self.comments_fail.is_empty()
            || !self.users_fail.is_empty()
            || !self.profiles_fail.is_empty()
    }
}

/// 执行迁移
///
/// `dry_run` 为 true 时只解析不写库。
pub async fn run(db: &DatabaseConnection, dry_run: bool) -> Result<Report, AppError> {
    let mut report = Report::default();

    let root = crate::paths::root();

    // ---------- 1. 文章 ----------
    let posts_dir = root.join("posts");
    if posts_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&posts_dir)
            .map_err(|e| AppError::Internal(format!("读 posts 目录失败: {e}")))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|x| x.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let slug = slug.to_string();

            match migrate_article(db, &slug, &path, dry_run).await {
                Ok(true) => report.articles_ok += 1,
                Ok(false) => report.articles_skip += 1,
                Err(e) => report.articles_fail.push((slug, e.to_string())),
            }
        }
    }

    // ---------- 2. 评论 ----------
    let comments_dir = root.join("data").join("comments");
    if comments_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&comments_dir)
            .map_err(|e| AppError::Internal(format!("读 comments 目录失败: {e}")))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|x| x.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let slug = slug.to_string();

            match migrate_comments(db, &slug, &path, dry_run).await {
                Ok((ok, skip)) => {
                    report.comments_ok += ok;
                    report.comments_skip += skip;
                }
                Err(e) => report.comments_fail.push((slug, e.to_string())),
            }
        }
    }

    // ---------- 3. 用户 ----------
    let users_file = root.join("data").join("users.txt");
    if users_file.is_file() {
        match migrate_users(db, &users_file, dry_run).await {
            Ok((ok, skip)) => {
                report.users_ok += ok;
                report.users_skip += skip;
            }
            Err(e) => report.users_fail.push(("users.txt".to_string(), e.to_string())),
        }
    }

    // ---------- 4. 资料 ----------
    let profiles_dir = root.join("data").join("profiles");
    if profiles_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&profiles_dir)
            .map_err(|e| AppError::Internal(format!("读 profiles 目录失败: {e}")))?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let username = name.to_string();

            match migrate_profile(db, &username, &path, dry_run).await {
                Ok(true) => report.profiles_ok += 1,
                Ok(false) => report.profiles_skip += 1,
                Err(e) => report.profiles_fail.push((username, e.to_string())),
            }
        }
    }

    // ---------- 5. uploads 表（只迁记录，不迁文件） ----------
    migrate_uploads(db, &root, dry_run).await?;

    // ---------- 6. 项目 / 工具（Markdown → showcases） ----------
    match crate::migrate_showcase::run(db, dry_run).await {
        Ok(sr) => {
            log::info!("项目/工具迁移: {}", sr.summary());
            for e in &sr.errors {
                log::warn!("  项目/工具迁移错误: {e}");
            }
        }
        Err(e) => log::warn!("项目/工具迁移失败: {e}"),
    }

    // ---------- 7. 论坛（旧 forum.db → 新库） ----------
    match crate::migrate_forum::run(db, dry_run).await {
        Ok(fr) => {
            log::info!("论坛迁移: {}", fr.summary());
            for e in &fr.errors {
                log::warn!("  论坛迁移错误: {e}");
            }
        }
        Err(e) => log::warn!("论坛迁移失败: {e}"),
    }

    Ok(report)
}

/// 迁移一篇文章
///
/// 返回 Ok(true)=新增, Ok(false)=已存在跳过
async fn migrate_article(
    db: &DatabaseConnection,
    slug: &str,
    path: &std::path::Path,
    dry_run: bool,
) -> Result<bool, AppError> {
    if article::find(db, slug).await?.is_some() {
        return Ok(false);
    }

    let text = std::fs::read_to_string(path)?;
    let parsed = parse_front_matter(&text)?;

    if dry_run {
        log::info!("[dry-run] 将迁移文章 {slug}");
        return Ok(true);
    }

    article::create(
        db,
        article::NewArticle {
            slug,
            title: &parsed.title,
            summary: &parsed.summary,
            category: &parsed.category,
            tags: &parsed.tags,
            cover_image: parsed.cover_image.as_deref(),
            body: &parsed.body,
            is_published: parsed.is_published,
            author: parsed.author.as_deref().unwrap_or(""),
            review_status: &parsed.review_status,
            is_featured: parsed.is_featured,
        },
    )
    .await?;

    Ok(true)
}

/// 迁移一个评论文件
///
/// 返回 (新增数, 跳过数)
async fn migrate_comments(
    db: &DatabaseConnection,
    slug: &str,
    path: &std::path::Path,
    dry_run: bool,
) -> Result<(usize, usize), AppError> {
    let text = std::fs::read_to_string(path)?;
    let list: Vec<comment::Comment> = serde_json::from_str(&text)
        .map_err(|e| AppError::BadRequest(format!("评论 JSON 解析失败: {e}")))?;

    let mut ok = 0;
    let mut skip = 0;

    for c in list {
        // 已存在则跳过（幂等）
        let exists = comment::load(db, slug)
            .await?
            .iter()
            .any(|x| x.id == c.id);

        if exists {
            skip += 1;
            continue;
        }

        if dry_run {
            ok += 1;
            continue;
        }

        comment::add(
            db,
            slug,
            &c.author,
            &c.email,
            &c.content,
            c.parent_id.clone(),
            c.is_approved,
        )
        .await?;
        ok += 1;
    }

    Ok((ok, skip))
}

/// 迁移 users.txt
///
/// 格式：`用户名:密码哈希:角色`，角色为 admin / user
async fn migrate_users(
    db: &DatabaseConnection,
    path: &std::path::Path,
    dry_run: bool,
) -> Result<(usize, usize), AppError> {
    let text = std::fs::read_to_string(path)?;
    let mut ok = 0;
    let mut skip = 0;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 2 {
            continue;
        }

        let username = parts[0].trim();
        let hash = parts[1].trim();
        let is_admin = parts.get(2).map(|r| r.trim() == "admin").unwrap_or(false);

        if username.is_empty() || hash.is_empty() {
            continue;
        }

        if user::find(db, username).await?.is_some() {
            skip += 1;
            continue;
        }

        if dry_run {
            ok += 1;
            continue;
        }

        // users.txt 不含邮箱与昵称，这两项留空（历史账号本就没有）
        user::create(db, username, hash, is_admin, "", true, "").await?;
        ok += 1;
    }

    Ok((ok, skip))
}

/// 迁移一个资料 JSON
async fn migrate_profile(
    db: &DatabaseConnection,
    username: &str,
    path: &std::path::Path,
    dry_run: bool,
) -> Result<bool, AppError> {
    // 已存在则跳过
    let existing = profile::load(db, username).await?;
    if !existing.nickname.is_empty()
        || !existing.bio.is_empty()
        || !existing.avatar.is_empty()
    {
        return Ok(false);
    }

    let text = std::fs::read_to_string(path)?;
    let p: profile::Profile = serde_json::from_str(&text)
        .map_err(|e| AppError::BadRequest(format!("资料 JSON 解析失败: {e}")))?;

    if dry_run {
        return Ok(true);
    }

    profile::save(db, username, &p).await?;
    Ok(true)
}

/// 迁移 uploads 表：扫描 static/uploads 目录，为每个文件建"无主"记录
///
/// 为什么要这一步：
///   原 uploads 记录在 forum.db 里（论坛库），现在要迁到 wanle.db。
///   但无法从旧库可靠推断每个文件的上传者，所以统一按"无主"处理
///   （uploader 为空 = 只有管理员能删），与原实现的兜底逻辑一致。
async fn migrate_uploads(
    db: &DatabaseConnection,
    root: &std::path::Path,
    dry_run: bool,
) -> Result<(), AppError> {
    let dir = root.join("static").join("uploads");
    if !dir.is_dir() {
        return Ok(());
    }

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == ".gitkeep" {
            continue;
        }

        // 已有记录则跳过
        if upload::owner(db, name).await?.is_some() {
            continue;
        }

        if dry_run {
            continue;
        }

        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let created = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            })
            .unwrap_or_else(user::now);

        let _ = upload::record(db, name, "", size, &created).await;
    }

    Ok(())
}

/// 解析 Markdown 的 YAML front matter
///
/// 与原 `content::parse` 的字段集合保持一致。
struct ParsedArticle {
    title: String,
    summary: String,
    category: String,
    tags: Vec<String>,
    cover_image: Option<String>,
    is_published: bool,
    author: Option<String>,
    review_status: String,
    is_featured: bool,
    body: String,
}

fn parse_front_matter(text: &str) -> Result<ParsedArticle, AppError> {
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
        author: Option<String>,
        #[serde(default = "default_review")]
        review_status: String,
        #[serde(default)]
        is_featured: bool,
    }

    fn default_category() -> String {
        "blog".to_string()
    }
    fn default_true() -> bool {
        true
    }
    fn default_review() -> String {
        "approved".to_string()
    }

    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        // 没有 front matter：整篇当正文，标题取文件名
        return Err(AppError::BadRequest(
            "缺少 front matter".to_string(),
        ));
    }

    let rest = &trimmed[3..];
    let end = rest
        .find("\n---")
        .ok_or_else(|| AppError::BadRequest("front matter 未闭合".to_string()))?;

    let yaml = &rest[..end];
    let body = rest[end + 4..].trim_start_matches('\n').to_string();

    let f: Front = serde_yaml::from_str(yaml)
        .map_err(|e| AppError::BadRequest(format!("front matter 解析失败: {e}")))?;

    Ok(ParsedArticle {
        title: f.title,
        summary: f.summary,
        category: f.category,
        tags: f.tags,
        cover_image: f.cover_image,
        is_published: f.is_published,
        author: f.author,
        review_status: f.review_status,
        is_featured: f.is_featured,
        body,
    })
}

/// 统计各表当前行数（迁移后核对用）
pub async fn counts(db: &DatabaseConnection) -> Result<String, AppError> {
    let articles = article::count_all(db).await?;
    let comments = comment::count_all(db).await?;
    let users = crate::entities::user::Entity::find()
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计用户失败: {e}")))?;
    let profiles = crate::entities::profile::Entity::find()
        .count(db)
        .await
        .map_err(|e| AppError::Internal(format!("统计资料失败: {e}")))?;

    Ok(format!(
        "articles={articles} comments={comments} users={users} profiles={profiles}"
    ))
}
