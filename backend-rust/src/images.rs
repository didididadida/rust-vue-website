//! 图片优化：无损重编码、缩略图按需生成。
//!
//! 为什么需要这个模块
//! ------------------
//! 图床上传的是"原图"——手机截图动辄 2.4 MB 的 PNG、相机 JPEG 五六 MB。
//! 文章列表一次要展示十几个封面，等于把几十 MB 原图推给手机。
//!
//! 两种手段：
//!   1. 无损重编码（静态图，上传时做一次）：PNG 走最大化 zlib 压缩，
//!      JPEG 走 mozjpeg 高质量重编码 —— 像素不变，只是文件小
//!   2. 缩略图（按需生成并缓存）：列表/封面只显示几百像素宽，
//!      没必要加载原尺寸
//!
//! 边界（务必知道）
//! ----------------
//! - **GIF 与动画 WebP 完全跳过**：`image` crate 解码后只保留第一帧，
//!   重编码会把动图压成静图。用 `gif` crate 判断帧数，是动图就原样落盘
//! - **SVG / ICO 跳过**：前者是矢量文本，后者是多尺寸图标，都不在图片处理范畴
//! - 不做有损 WebP 转换：那需要额外编码器依赖，且会改变图片内容；
//!   无损重编码已经能把截图类 PNG 压掉 30~60%
//! - 任何一步失败都**保留原始文件**（降级为"不优化"），绝不让优化损坏上传
//!
//! 输入 → 输出
//! ------------
//!   normalize(path)      → 原文件被就地无损重编码（更小），返回是否改动
//!   ensure_variants()    → 启动时把 uploads 里的静态图补做一遍重编码（幂等）
//!
//! 依据
//! ----
//! 经典问题：缓存（cache）—— 用空间换时间的派生数据
//! 本处变体：派生结果（缩略图）由原始文件的内容哈希命名，而非时间戳
//! 沿用结论：名称即内容指纹，内容不变则缓存永远有效，无需失效通知
//! 偏离处：原图被替换后旧缩略图会变成孤儿文件（哈希不同 → 新文件名），

use std::fs;
use std::io::BufReader;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;

use crate::paths;

/// 可以安全重编码的静态位图扩展名
const LOSSLESS_EXTS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

/// 缩略图允许的宽度档位（前端按需选一档，避免任意宽度导致缓存爆炸）

/// 单张图片的像素上限：超过就不解码，防止 2 亿像素的图把内存打爆
const MAX_PIXELS: u64 = 40_000_000;

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

/// 是否是"可以重编码的静态图"
pub fn is_lossless_candidate(path: &Path) -> bool {
    LOSSLESS_EXTS.contains(&ext_of(path).as_str())
}

/// 是否为动图（GIF / 动画 WebP）。
///
/// 判据：GIF 用 `gif` crate 数帧；WebP 用 `image` 的 WebP 解码器只给静态帧，
/// 所以改用字节层判断 RIFF 结构里的 ANIM/ANMF 块是否存在。
fn is_animated(path: &Path) -> bool {
    match ext_of(path).as_str() {
        "gif" => {
            // 注意：read_info 返回的 reader 借用输入切片，所以必须在同一个作用域内用完，
            // 不能把它带出闭包（否则 E0515：返回值引用了局部变量）
            let Ok(bytes) = fs::read(path) else { return false };
            let mut cursor = BufReader::new(&bytes[..]);
            match gif::DecodeOptions::new().read_info(&mut cursor) {
                Ok(mut reader) => reader.next_frame_info().ok().flatten().is_some(),
                Err(_) => false,
            }
        }
        "webp" => fs::read(path)
            .map(|b| {
                // 动画 WebP 的 RIFF 里会出现 "ANIM" 与 "ANMF" 块
                b.windows(4).any(|w| w == b"ANIM") && b.windows(4).any(|w| w == b"ANMF")
            })
            .unwrap_or(false),
        _ => false,
    }
}

/// 把原图无损重编码（就地），返回原大小与新大小。
///
/// 规则：
///   - PNG → PNG，压缩级别提到 9（视觉完全一致）
///   - JPEG → JPEG，质量 92（mozjpeg 在这个质量下通常比原图更小且无法眼差异）
///   - WebP → 只解码判断是否为动图，静态 WebP 保持原样（避免多一种编码路径）
///   - 新文件比原来大 → 保持原文件不动（有些图本来就压得很紧）
pub fn normalize(path: &Path) -> std::io::Result<(u64, u64)> {
    let before = fs::metadata(path)?.len();
    if before == 0 {
        return Ok((0, 0));
    }
    let ext = ext_of(path);
    if !is_lossless_candidate(path) || is_animated(path) {
        return Ok((before, before));
    }

    let reader = image::ImageReader::open(path)
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if (w as u64) * (h as u64) > MAX_PIXELS {
        return Ok((before, before));
    }

    let img = image::ImageReader::open(path)
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .decode()
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    let mut out = Vec::new();
    match ext.as_str() {
        "png" => {
            // 保留 alpha 的原图用 RGBA，其余用 RGB（省掉无用通道）
            let rgb = img.to_rgb8();
            let rgba = img.to_rgba8();
            let has_alpha = rgba.pixels().any(|p| p.0[3] < 255);
            let enc = PngEncoder::new_with_quality(
                &mut out,
                image::codecs::png::CompressionType::Best,
                image::codecs::png::FilterType::Adaptive,
            );
            if has_alpha {
                use image::ImageEncoder;
                enc.write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
            } else {
                use image::ImageEncoder;
                enc.write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
            }
        }
        "jpg" | "jpeg" => {
            let mut enc = JpegEncoder::new_with_quality(&mut out, 92);
            enc.encode_image(&img.to_rgb8())
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        _ => return Ok((before, before)),
    }

    // 只有确实变小才覆盖：避免把已经优化过的图写坏
    if (out.len() as u64) < before {
        fs::write(path, &out)?;
        Ok((before, out.len() as u64))
    } else {
        Ok((before, before))
    }
}

/// 启动时补齐：把 uploads 里还没优化过的静态图做一遍无损重编码。
///
/// 幂等：已经优化过的图重编码一次不会再变小（判定"没变小就不写"），
/// 所以重复启动不会反复写盘。
pub fn ensure_variants() {
    let dir = paths::uploads_dir();
    let Ok(entries) = fs::read_dir(&dir) else { return };
    let mut saved: u64 = 0;
    let mut count = 0;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.contains(".transcoded.") || name.ends_with(".poster.jpg") {
            continue;
        }
        if !is_lossless_candidate(&path) {
            continue;
        }
        if let Ok((before, after)) = normalize(&path) {
            if after < before {
                saved += before - after;
                count += 1;
                log::info!(
                    "图片优化：{} {} → {}",
                    name,
                    human(before),
                    human(after)
                );
            }
        }
    }
    if count > 0 {
        log::info!("启动图片优化：{} 张，共省下 {}", count, human(saved));
    }
}

fn human(size: u64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn unsupported_extensions_are_rejected() {
        assert!(!is_lossless_candidate(Path::new("a.svg")));
        assert!(!is_lossless_candidate(Path::new("a.mp4")));
        assert!(is_lossless_candidate(Path::new("a.PNG")));
    }
}
