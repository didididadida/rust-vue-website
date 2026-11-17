//! 图床：上传、列表、删除。
//!
//! 权限模型：
//! - 上传：任何已登录用户
//! - 删除：上传者本人 或 站长（admin）
//! - 历史图片（归属表无记录）：只有站长能删，普通用户看不到删除按钮
//!
//! 依据：
//!   经典问题：资源所有权校验（ownership check / 越权访问 IDOR）
//!   本处变体：文件系统只存文件名，没有天然的所有者概念；
//!             权限信息必须落到独立的地方，且必须与文件本体保持同步
//!   沿用结论：每次删除都在服务端重新读归属再判定，前端按钮状态只作提示，
//!             即使前端被绕过，服务端仍会拒绝
//!   偏离处：归属表比文件晚写入时用「删库行」回滚；反过来（文件已删、库行残留）
//!           由 forget_upload 在删除路径上清掉，两边最终一致

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::stream::TryStreamExt;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;

use crate::content;
use crate::db;
use crate::errors::AppError;
use crate::images;
use crate::middleware;
use crate::models::{ApiResponse, ImageInfo};
use crate::paths;

/// 允许上传的图片扩展名
const ALLOWED_IMAGE: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico"];

/// 允许上传的视频扩展名
const ALLOWED_VIDEO: [&str; 6] = ["mp4", "webm", "ogv", "ogg", "mov", "m4v"];

/// 视频体积上限（字节）。超过时在写盘前就拒绝，避免把磁盘写满
const MAX_VIDEO_BYTES: u64 = 200 * 1024 * 1024;

fn human_size(size: u64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)
    }
}

fn modified_at(meta: &fs::Metadata) -> String {
    let time = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let dt: chrono::DateTime<chrono::Utc> = time.into();
    dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// 校验并拼出磁盘路径（拒绝任何路径穿越）
fn safe_path(filename: &str) -> Result<std::path::PathBuf, AppError> {
    if !content::is_safe_slug(filename)
        || Path::new(filename).file_name().and_then(|f| f.to_str()) != Some(filename)
    {
        return Err(AppError::BadRequest("文件名不合法".to_string()));
    }
    Ok(paths::uploads_dir().join(filename))
}

/// 删除权限：站长全权；其余人只能删自己上传且归属表里有记录的图
///
/// 输入 → 输出：(当前用户, 上传者) → 能否删除
/// 不变量：uploader 为空（历史图片）时只有 admin 通过
/// 边界：用户名大小写不同视为同一人（与论坛作者判定保持一致）
fn may_delete(is_admin: bool, requester: &str, uploader: &str) -> bool {
    if is_admin {
        return true;
    }
    !uploader.is_empty() && uploader.eq_ignore_ascii_case(requester)
}

/// POST /api/upload
pub async fn upload_file(
    mut payload: Multipart,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    let dir = paths::uploads_dir();
    fs::create_dir_all(&dir)?;

    while let Some(mut field) = payload.try_next().await? {
        let disposition = field
            .content_disposition()
            .cloned()
            .ok_or_else(|| AppError::BadRequest("缺少 Content-Disposition".to_string()))?;

        // 只处理表单里的 file 字段
        if let Some(name) = disposition.get_name() {
            if name != "file" {
                continue;
            }
        }

        let original = disposition.get_filename().unwrap_or("").to_string();
        let ext = Path::new(&original)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let is_image = ALLOWED_IMAGE.contains(&ext.as_str());
        let is_video = ALLOWED_VIDEO.contains(&ext.as_str());
        if !is_image && !is_video {
            let shown = if ext.is_empty() {
                "未知"
            } else {
                ext.as_str()
            };
            return Err(AppError::BadRequest(format!(
                "不支持的文件格式：{}（图片 {}，视频 {}）",
                shown,
                ALLOWED_IMAGE.join(" / "),
                ALLOWED_VIDEO.join(" / ")
            )));
        }

        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let short = uuid::Uuid::new_v4().simple().to_string();
        let new_name = format!("{}-{}.{}", stamp, &short[..8], ext);
        let filepath = dir.join(&new_name);

        // 视频单独限流：单文件上限 + 边写边计数，超限立刻中止并清理
        let limit = if is_video {
            Some(MAX_VIDEO_BYTES)
        } else {
            None
        };

        // 先写文件，再落归属记录；归属写失败就把刚写的文件删掉，
        // 宁可让这次上传失败，也不留下"没有主人"的文件
        let written = write_file(&mut field, &filepath, limit).await;
        let raw_size = match written {
            Ok(size) => size,
            Err(e) => {
                let _ = fs::remove_file(&filepath);
                return Err(e);
            }
        };

        // 图片优化：静态图就地无损重编码（PNG/JPEG），动图与 SVG/ICO 原样保留。
        // 失败一律降级——原图仍然是完整可用的，不能因为"想优化"让上传失败。
        // 注意：无损重编码不改变格式，所以扩展名不变，文件名无需改动。
        let mut size = raw_size;
        if is_image && images::is_lossless_candidate(&filepath) {
            match images::normalize(&filepath) {
                Ok((before, after)) if after < before => {
                    size = after;
                    log::info!(
                        "图片优化：{} {} → {}",
                        new_name,
                        human_size(before),
                        human_size(after)
                    );
                }
                // 没变小（本来压得就紧）或无需处理：保持原样
                Ok(_) => {}
                Err(e) => log::warn!("图片优化失败，保留原图 {}: {}", new_name, e),
            }
        }

        if let Err(e) = db::record_upload(&new_name, &claims.sub, size, &content::now()) {
            let _ = fs::remove_file(&filepath);
            return Err(e);
        }

        let kind = if is_video { "视频" } else { "图片" };
        log::info!(
            "{} 上传了{} {}（{}）",
            claims.sub,
            kind,
            filepath.display(),
            human_size(size)
        );

        // 视频：入转码队列后立即返回，前端先按原始地址播放，
        // 转码完成后 /api/images 会带上 transcoded_url，前端自动切过去
        let mut payload = serde_json::json!({
            "url": format!("/static/uploads/{}", new_name),
            "filename": new_name,
            "uploader": claims.sub,
            "size": size,
            "kind": if is_video { "video" } else { "image" }
        });
        // 图片额外给一个缩略图地址：编辑器/图床列表可以直接先用小图占位，
        // 省掉一次"先等大图再缩"的等待。视频的封面由转码队列产出，不在这里。
        if is_image && !is_video {
        }
        if is_video {
            crate::transcode::enqueue(filepath.clone());
            payload["transcoding"] = serde_json::json!(crate::transcode::ffmpeg_available());
        }

        return Ok(HttpResponse::Ok().json(ApiResponse::success(payload)));
    }

    Err(AppError::BadRequest("未找到上传文件".to_string()))
}

/// 把 multipart 字段落到磁盘，返回字节数。
///
/// `limit` 为 Some 时边写边累计，超过上限立即中止。
/// 依据：
///   经典问题：流式写入的长度上限（streaming with a bounded sink）
///   本处变体：multipart 分块到达，无法在写之前知道总长度
///   沿用结论：按块累加并在越界时提前失败，避免把整段内容读进内存再判断
///   偏离处：越界时磁盘上会短暂存在半截文件，由调用方负责删除
async fn write_file(
    field: &mut actix_multipart::Field,
    filepath: &Path,
    limit: Option<u64>,
) -> Result<u64, AppError> {
    let mut file = fs::File::create(filepath)?;
    let mut size: u64 = 0;
    while let Some(chunk) = field.try_next().await? {
        size += chunk.len() as u64;
        if let Some(max) = limit {
            if size > max {
                return Err(AppError::BadRequest(format!(
                    "文件超过大小上限（{}）",
                    human_size(max)
                )));
            }
        }
        file.write_all(&chunk)?;
    }
    file.flush()?;
    Ok(size)
}

/// GET /api/images —— 列出图片，并标注归属与当前用户能否删除
pub async fn list_images(req: HttpRequest) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    // 一次取出自己上传过的文件名集合，避免每张图查一次库
    let mine: HashSet<String> = if claims.is_admin {
        HashSet::new()
    } else {
        db::uploads_of(&claims.sub)?
    };

    let mut images: Vec<(SystemTime, ImageInfo)> = Vec::new();
    if let Ok(entries) = fs::read_dir(paths::uploads_dir()) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let filename = entry.file_name().to_string_lossy().to_string();
            // 转码产物（.transcoded.mp4 / .poster.jpg）不作为独立条目展示，
            // 它们随原标题一起出现在 transcoded_url / poster_url 里。
            // 隐藏的占位文件（.gitkeep 等）也不该出现在图床列表
            if filename.contains(".transcoded.")
                || filename.ends_with(".poster.jpg")
                || filename.starts_with('.')
            {
                continue;
            }
            let uploader = db::find_upload_owner(&filename)
                .unwrap_or(None)
                .unwrap_or_default();
            // 站长全开；普通用户只要文件名出现在自己的集合里即可删
            let can_delete = claims.is_admin || mine.contains(&filename);
            // 视频若有转码产物，额外给出地址（前端优先播放转码后的 mp4）
            let transcoded_url = crate::transcode::transcoded_url(&filename);
            // 是否视频：看扩展名
            let ext = std::path::Path::new(&filename)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            let is_video = ALLOWED_VIDEO.contains(&ext.as_str());
            // 视频封面：转码时生成的 xxx.poster.jpg，存在才给地址
            let poster_url = if is_video {
                let poster = crate::transcode::poster_path(&paths::uploads_dir().join(&filename));
                if poster.is_file() {
                    let pname = poster
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    Some(format!("/static/uploads/{}", pname))
                } else {
                    None
                }
            } else {
                None
            };
            images.push((
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                ImageInfo {
                    url: format!("/static/uploads/{}", filename),
                    filename,
                    size: meta.len(),
                    size_human: human_size(meta.len()),
                    modified_at: modified_at(&meta),
                    uploader,
                    can_delete,
                    transcoded_url,
                    is_video,
                    poster_url,
                },
            ));
        }
    }

    images.sort_by(|a, b| b.0.cmp(&a.0));
    let list: Vec<ImageInfo> = images.into_iter().map(|(_, info)| info).collect();
    Ok(HttpResponse::Ok().json(ApiResponse::success(list)))
}


/// DELETE /api/images/{filename} —— 上传者本人或站长
pub async fn delete_image(
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = middleware::verify_user(req)?;

    let filename = path.into_inner();
    let filepath = safe_path(&filename)?;

    // 先取归属再判权限：文件不存在时直接 404，不泄露归属信息
    if !filepath.is_file() {
        return Err(AppError::NotFound("图片不存在".to_string()));
    }
    let uploader = db::find_upload_owner(&filename)?.unwrap_or_default();
    if !may_delete(claims.is_admin, &claims.sub, &uploader) {
        let message = if uploader.is_empty() {
            "这张图没有归属记录，只有站长可以删除"
        } else {
            "只能删除自己上传的图片"
        };
        return Err(AppError::Forbidden(message.to_string()));
    }

    // 先删文件再删归属行：万一中间失败，残留的库行会在下次删除同一文件时被清掉，
    // 而反过来（先删库行、文件删除失败）会留下永远删不掉的无主文件
    fs::remove_file(&filepath)?;
    let _ = db::forget_upload(&filename);
    log::info!("{} 删除了图片 {}", claims.sub, filepath.display());
    Ok(HttpResponse::Ok().json(ApiResponse::success(
        serde_json::json!({ "filename": filename }),
    )))
}
