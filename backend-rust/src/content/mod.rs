//! Markdown + YAML front matter 的极简内容存储层。
//!
//! 目录结构约定（每一项都是一篇 `.md`，文件名即 slug）：
//!
//! ```text
//! ---
//! title: 标题
//! tags: [博客, 随笔]
//! is_published: true
//! created_at: 2026-08-25T01:33:00Z
//! ---
//!
//! 正文（Markdown）
//! ```

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::errors::AppError;

/// 文件名中不允许出现的字符
const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

// ==================== 记录结构 ====================

fn default_true() -> bool {
    true
}

/// 兼容两种写法：
/// ```yaml
/// tags: [博客, 随笔]     # 数组
/// tags: 博客, 随笔       # 逗号/顿号分隔的字符串
/// ```
fn de_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrList {
        List(Vec<String>),
        Text(String),
    }

    let value = Option::<StringOrList>::deserialize(deserializer)?;
    Ok(match value {
        None => Vec::new(),
        Some(StringOrList::List(list)) => clean_list(Some(list)),
        Some(StringOrList::Text(text)) => clean_list(Some(
            text.split([',', '，', '、', ';', '；'])
                .map(|s| s.to_string())
                .collect(),
        )),
    })
}

/// 博客文章
///
/// 新增三类字段用于「社区投稿」：
/// - `author`：作者用户名。为空表示本站作者（管理员），兼容历史文章
/// - `review_status`：pending（待审核）/ approved（已通过）/ rejected（已拒绝）
///   历史文章缺省为 approved，不会因为加了这个字段就集体变成待审核
/// - `is_featured`：是否在首页展示。管理员可手工指定，
///   也可由"审核通过且被推荐"自动决定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleRecord {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "de_string_list"
    )]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<String>,
    #[serde(default = "default_true")]
    pub is_published: bool,
    /// 作者用户名；留空 = 本站作者（管理员）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// 审核状态
    #[serde(default = "default_approved")]
    pub review_status: String,
    /// 是否在首页展示（仅对非本站作者的文章有意义）
    #[serde(default)]
    pub is_featured: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

fn default_approved() -> String {
    "approved".to_string()
}

impl ArticleRecord {
    /// 是否本站作者（管理员）的文章
    pub fn is_own(&self) -> bool {
        self.author
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
    }

    /// 是否已审核通过
    pub fn is_approved(&self) -> bool {
        self.review_status.eq_ignore_ascii_case("approved")
    }

    /// 是否应出现在首页：本站作者的都上；他人文章需审核通过且被推荐
    pub fn show_on_home(&self) -> bool {
        self.is_own() || (self.is_approved() && self.is_featured)
    }
}

/// 一篇已读取的文档
#[derive(Debug, Clone)]
pub struct Doc<T> {
    pub slug: String,
    pub data: T,
    pub body: String,
}

// ==================== 工具函数 ====================

/// UTC 时间戳（RFC3339，带 Z 后缀，前端可直接 new Date()）
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// slug 是否可以作为文件名安全使用
pub fn is_safe_slug(slug: &str) -> bool {
    let s = slug.trim();
    if s.is_empty() || s.chars().count() > 120 {
        return false;
    }
    if s.starts_with('.') || s.ends_with('.') || s.ends_with(' ') {
        return false;
    }
    if s.contains("..") {
        return false;
    }
    !s.chars().any(|c| c.is_control() || FORBIDDEN.contains(&c))
}

/// 由标题生成 slug；若标题里没有任何可用字符（例如纯符号），返回随机 slug
pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in input.trim().chars() {
        if ch.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
            pending_dash = false;
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        random_slug()
    } else {
        out
    }
}

fn random_slug() -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!("item-{}", &hex[..8])
}

/// 清洗标签/技术栈：去空白、去空项、去重
pub fn clean_list(items: Option<Vec<String>>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items.unwrap_or_default() {
        let v = item.trim().to_string();
        if !v.is_empty() && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// 清洗可选文本：空白视为“未设置”
pub fn clean_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// ==================== 读写 ====================

fn file_path(dir: &Path, slug: &str) -> Result<PathBuf, AppError> {
    if !is_safe_slug(slug) {
        return Err(AppError::BadRequest(format!("非法的标识: {}", slug)));
    }
    Ok(dir.join(format!("{}.md", slug.trim())))
}

pub fn exists(dir: &Path, slug: &str) -> bool {
    file_path(dir, slug).map(|p| p.is_file()).unwrap_or(false)
}

/// 解析 `---` front matter + 正文
pub fn parse<T: DeserializeOwned>(raw: &str) -> Result<(T, String), AppError> {
    let raw = raw.trim_start_matches('\u{feff}').trim();
    if !raw.starts_with("---") {
        return Err(AppError::BadRequest("缺少 YAML front matter".to_string()));
    }
    let rest = &raw[3..];
    let end = rest
        .find("\n---")
        .ok_or_else(|| AppError::BadRequest("front matter 未正确闭合".to_string()))?;
    let yaml = &rest[..end];
    let body = rest[end + 4..]
        .trim_start_matches(['\r', '\n'])
        .trim_end()
        .to_string();
    let data = serde_yaml::from_str::<T>(yaml)
        .map_err(|e| AppError::BadRequest(format!("front matter 解析失败: {}", e)))?;
    Ok((data, body))
}

/// 序列化为带 front matter 的 Markdown 文本
pub fn render<T: Serialize>(data: &T, body: &str) -> Result<String, AppError> {
    let yaml = serde_yaml::to_string(data)
        .map_err(|e| AppError::Internal(format!("front matter 生成失败: {}", e)))?;
    let mut out = String::from("---\n");
    out.push_str(&yaml);
    out.push_str("---\n");
    let body = body.trim();
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
        out.push('\n');
    }
    Ok(out)
}

/// 读取目录下所有 `.md`；无法解析的文件跳过并记录警告
pub fn list<T: DeserializeOwned>(dir: &Path) -> Result<Vec<Doc<T>>, AppError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .map(|ext| ext.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
        })
        .collect();
    paths.sort();

    let mut docs = Vec::new();
    for path in paths {
        let slug = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(e) => {
                log::warn!("读取失败 {}: {}", path.display(), e);
                continue;
            }
        };
        match parse::<T>(&raw) {
            Ok((data, body)) => docs.push(Doc { slug, data, body }),
            Err(e) => log::warn!("跳过无法解析的文件 {}: {}", path.display(), e),
        }
    }
    Ok(docs)
}

/// 按 slug 读取单篇
pub fn find<T: DeserializeOwned>(dir: &Path, slug: &str) -> Result<Option<Doc<T>>, AppError> {
    let path = file_path(dir, slug)?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let (data, body) = parse::<T>(&raw).map_err(|e| {
        AppError::Internal(format!("解析 {} 失败: {}", path.display(), e.message()))
    })?;
    Ok(Some(Doc {
        slug: slug.trim().to_string(),
        data,
        body,
    }))
}

/// 写入（新建或覆盖）
pub fn save<T: Serialize>(dir: &Path, slug: &str, data: &T, body: &str) -> Result<(), AppError> {
    let path = file_path(dir, slug)?;
    fs::create_dir_all(dir)?;
    fs::write(&path, render(data, body)?)?;
    log::info!("已写入 {}", path.display());
    Ok(())
}

/// 读取某个 slug 对应文件的**原始全文**（含 front matter 那一段 `---`）。
///
/// 与 `find` 的区别：`find` 把 YAML 解析成结构体、把正文单独返回；
/// 这里原样给出磁盘上的字节内容，供"直接编辑整个 .md"的编辑器使用。
pub fn read_raw(dir: &Path, slug: &str) -> Result<Option<String>, AppError> {
    let path = file_path(dir, slug)?;
    match fs::read_to_string(&path) {
        Ok(raw) => Ok(Some(raw)),
        Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 直接把**原始文本**写回某个 slug 的文件，不重新序列化任何字段。
///
/// 输入 → 输出：slug + 完整 .md 文本 → 覆盖磁盘文件
/// 关键机制：不做任何 YAML 重排，保留用户手写的缩进、字段顺序与注释
/// 不变量：写入前必须已通过 `parse` 校验（由调用方保证），否则文章会"消失"
/// 边界：不创建文件——只用于覆盖已存在的文章，新建走 upsert 接口
///
/// 依据：
///   经典问题：序列化往返的有损性（round-trip 不保真）
///   本处变体：用户直接在 front matter 里手写 YAML，经过结构体再序列化会
///             丢失字段顺序、注释与格式，把"我写的文件"变成"系统生成的文件"
///   沿用结论：能原样传就原样传，只在边界做校验，不做中间态转换
///   偏离处：放弃了结构体层的字段归一（如 tags 去重由前端/用户自己负责）
pub fn write_raw(dir: &Path, slug: &str, raw: &str) -> Result<(), AppError> {
    let path = file_path(dir, slug)?;
    fs::write(&path, raw)?;
    log::info!("已写入（原文）{}", path.display());
    Ok(())
}

/// 删除，返回是否真的删掉了文件
pub fn remove(dir: &Path, slug: &str) -> Result<bool, AppError> {
    let path = file_path(dir, slug)?;
    match fs::remove_file(&path) {
        Ok(_) => {
            log::info!("已删除 {}", path.display());
            Ok(true)
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

