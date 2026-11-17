//! 图床元数据库：SQLite（`data/wanle.db`），使用连接池。
//!
//! 逻辑描述：
//!   输入 → 输出：上传的文件名 + 上传者 → 写入 `uploads` 表；查询时按文件名/上传者取回
//!   关键机制：WAL 模式 + busy_timeout，让读写不互相阻塞
//!   不变量：同一文件名只可能有一个上传者（filename 是主键）
//!   边界：找不到记录时返回 None（历史图片 / 管理员手工放入的图片）
//!
//! 为什么上传记录用数据库而不是文件：
//!   上传是**高频写**（每次插图一次），且需要按上传者筛选（`uploads_of`），
//!   用 SQLite 的索引比遍历目录快得多。
//!
//! 依据：
//!   经典问题：按访问模式选择数据结构（读写模式决定存储选型）
//!   来源：SQLite 官方文档 WAL 模式（"WAL is significantly faster in most scenarios"）
//!        https://sqlite.org/wal.html
//!   原型结论：WAL 模式下读不被写阻塞，适合"读多写少但写要快的元数据表"
//!   本处变体：单机、单进程、无并发写入压力，仅需一个带索引的小表
//!   沿用结论：WAL + busy_timeout 足以覆盖本场景的并发形态
//!   偏离处：未做连接池上限调优（固定 8），个人站并发远低于此

use std::sync::OnceLock;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::errors::AppError;
use crate::paths;

pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConn = r2d2::PooledConnection<SqliteConnectionManager>;

static POOL: OnceLock<DbPool> = OnceLock::new();

fn db_path() -> std::path::PathBuf {
    paths::data_dir().join("wanle.db")
}

/// 初始化连接池与表结构（在 main 启动时调用一次）
pub fn init() -> Result<(), AppError> {
    let path = db_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }

    let manager = SqliteConnectionManager::file(&path).with_init(|c| {
        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;",
        )
    });

    let pool = Pool::builder()
        .max_size(8)
        .build(manager)
        .map_err(|e| AppError::Internal(format!("数据库连接池创建失败: {}", e)))?;

    {
        let conn = pool
            .get()
            .map_err(|e| AppError::Internal(format!("获取数据库连接失败: {}", e)))?;
        migrate(&conn)?;
    }

    let _ = POOL.set(pool);
    Ok(())
}

/// 取一个连接
pub fn conn() -> Result<DbConn, AppError> {
    POOL.get()
        .ok_or_else(|| AppError::Internal("数据库未初始化".to_string()))?
        .get()
        .map_err(|e| AppError::Internal(format!("获取数据库连接失败: {}", e)))
}

/// 建表（幂等，可重复执行）
fn migrate(conn: &rusqlite::Connection) -> Result<(), AppError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS uploads (
            filename    TEXT    PRIMARY KEY,
            uploader    TEXT    NOT NULL,
            size        INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT    NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_uploads_uploader ON uploads(uploader, created_at DESC);
        "#,
    )?;

    // 老版本可能没有 uploads 表，也无从知道历史图片是谁传的：
    // 按「无主」处理（uploader 留空），只有管理员能删，避免误删别人正在用的图。
    Ok(())
}

// ==================== 上传记录 ====================

/// 记录一次上传
pub fn record_upload(
    filename: &str,
    uploader: &str,
    size: u64,
    created_at: &str,
) -> Result<(), AppError> {
    let conn = conn()?;
    conn.execute(
        "INSERT OR REPLACE INTO uploads (filename, uploader, size, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![filename, uploader, size as i64, created_at],
    )?;
    Ok(())
}

/// 删除上传记录（图片被删时调用）
pub fn forget_upload(filename: &str) -> Result<(), AppError> {
    let conn = conn()?;
    conn.execute("DELETE FROM uploads WHERE filename = ?1", params![filename])?;
    Ok(())
}

/// 查单张图片的归属
///
/// 返回 `Ok(Some(uploader))` 表示有记录；
/// `Ok(None)` 表示无记录（历史图片 / 管理员手工放入）。
pub fn find_upload_owner(filename: &str) -> Result<Option<String>, AppError> {
    let conn = conn()?;
    let owner: Option<String> = conn
        .query_row(
            "SELECT uploader FROM uploads WHERE filename = ?1",
            params![filename],
            |r| r.get(0),
        )
        .ok();
    Ok(owner)
}

/// 某个上传者传过的全部文件名
pub fn uploads_of(uploader: &str) -> Result<std::collections::HashSet<String>, AppError> {
    let conn = conn()?;
    let mut stmt = conn.prepare("SELECT filename FROM uploads WHERE uploader = ?1")?;
    let rows = stmt.query_map(params![uploader], |r| r.get::<_, String>(0))?;
    let mut out = std::collections::HashSet::new();
    for row in rows {
        out.insert(row?);
    }
    Ok(out)
}
