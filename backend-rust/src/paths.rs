//! 统一管理文件存储路径。
//!
//! 默认约定：在 `backend-rust/` 目录下启动服务，因此项目根目录为 `..`。
//! 每一项都可以用环境变量覆盖，方便换机部署。

use std::env;
use std::path::PathBuf;

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    match env::var(key) {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => default,
    }
}

/// 项目根目录（默认 `..`，即 backend-rust 的上一级）
pub fn root() -> PathBuf {
    match env::var("WANLE_ROOT") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => PathBuf::from(".."),
    }
}

/// 博客文章目录（Markdown + YAML front matter）
pub fn posts_dir() -> PathBuf {
    env_path("POSTS_DIR", root().join("posts"))
}

/// 数据目录（存放 users.txt / comments / wanle.db）
pub fn data_dir() -> PathBuf {
    env_path("DATA_DIR", root().join("data"))
}


/// 用户表（纯文本，一行一个用户）
pub fn users_file() -> PathBuf {
    env_path("USERS_FILE", data_dir().join("users.txt"))
}

/// 评论目录（每篇文章一个 `<slug>.json`）
pub fn comments_dir() -> PathBuf {
    env_path("COMMENTS_DIR", data_dir().join("comments"))
}

/// 静态资源根目录（由后端 /static 提供）
pub fn static_dir() -> PathBuf {
    env_path("STATIC_DIR", root().join("static"))
}

/// 图床上传目录
pub fn uploads_dir() -> PathBuf {
    env_path("UPLOADS_DIR", static_dir().join("uploads"))
}



/// 启动时确保所有数据目录存在
pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [
        posts_dir(),
        comments_dir(),
        uploads_dir(),
        users_file()
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(data_dir),
    ] {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(&dir)?;
        }
    }
    Ok(())
}
