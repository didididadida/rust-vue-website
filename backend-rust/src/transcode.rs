//! 视频转码队列
//!
//! 为什么要有队列：
//!   ffmpeg 转码是 CPU 密集型、耗时几十秒到几分钟的操作，绝对不能放在请求线程里同步做。
//!   上传请求只负责"存原始文件 + 入队"，转码在后台按 FIFO 串行执行。
//!
//! 设计要点：
//!   1. 单 worker 串行 —— 2 核小机器上并行转码会互相抢 CPU，串行反而总吞吐更稳
//!   2. 幂等     —— 同一文件重复入队只执行一次（用 in-flight 集合去重）
//!   3. 可降级   —— 机器上没装 ffmpeg 时整个队列空转，上传照常成功，只是不转码
//!   4. 失败即弃 —— 转码失败就保留原始文件，前端仍能直接播放 mp4/webm
//!
//! 产物：
//!   <uploads>/<name>.mp4      转码后的 720p H.264（faststart，便于边下边播）
//!   <uploads>/<name>.poster.jpg 首帧封面
//!   原始文件保留，转码只是"额外产出更快更通用的版本"
//!
//! 依据：
//!   经典问题：生产者–消费者解耦（后台任务队列）
//!   本处变体：生产端是 HTTP 请求，消费端是子进程 ffmpeg；需要落盘的结果而不是内存消息
//!   沿用结论：请求线程只入队即可返回，转码不被网络超时和客户端断开影响
//!   偏离处：没有持久化队列 —— 服务重启时正在排队的任务会丢，
//!           但这些文件本就有可播放的原件，丢任务只是"没转码"而非"内容丢失"

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::paths;

/// 转码目标分辨率上限（长边不超过这个值，等比缩放，不放大）
const MAX_LONG_EDGE: u32 = 1280;

/// 单个转码任务的最长运行时间
const TRANSCODE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

static QUEUE: OnceLock<Arc<Mutex<QueueState>>> = OnceLock::new();

#[derive(Default)]
struct QueueState {
    pending: Vec<PathBuf>,
    /// 正在排队的文件（去重用）
    known: HashSet<String>,
    worker_running: bool,
}

fn state() -> &'static Arc<Mutex<QueueState>> {
    QUEUE.get_or_init(|| Arc::new(Mutex::new(QueueState::default())))
}

/// 队列状态快照（给管理接口 / 日志用）
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueStatus {
    pub enabled: bool,
    pub pending: usize,
    pub running: bool,
}

/// ffmpeg 是否可用（结果缓存，避免每次都 fork 一个进程探测）
pub fn ffmpeg_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let ok = std::process::Command::new(ffmpeg_bin())
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            log::info!("检测到 ffmpeg，视频转码队列已启用");
        } else {
            log::warn!("未检测到 ffmpeg，视频将跳过转码（原始文件仍可播放）");
        }
        ok
    })
}

/// 选中可用的 H.264 编码器
///
/// 为什么需要探测：
///   发行版打包的 ffmpeg 未必带 libx264（GPL 部分常被剥离），例如
///   OpenCloudOS/EPEL 的 7.0.2 就只有 libopenh264。写死 libx264 会在某些机器上
///   直接失败。这里按优先级探测一次并缓存结果。
///
/// 依据：
///   经典问题：能力探测优于假设（capability detection）
///   本处变体：编码器集合随构建选项变化，无法在编译期确定
///   沿用结论：运行时探测 + 缓存，失败时按优先级降级
///   偏离处：两个编码器的参数不通用（x264 有 -preset/-crf，openh264 只有 -b:v），
///           所以返回编码器名后由调用方选择对应的参数组
fn pick_h264_encoder() -> Option<&'static str> {
    static PICKED: OnceLock<Option<&'static str>> = OnceLock::new();
    *PICKED.get_or_init(|| {
        if let Ok(name) = std::env::var("FFMPEG_ENCODER") {
            if !name.trim().is_empty() {
                let leaked: &'static str = Box::leak(name.trim().to_string().into_boxed_str());
                log::info!("按 FFMPEG_ENCODER 指定编码器: {}", leaked);
                return Some(leaked);
            }
        }

        let available = std::process::Command::new(ffmpeg_bin())
            .arg("-hide_banner")
            .arg("-encoders")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        // 注意：编码器列表是" V....D name 描述"的格式，
        // 不能用 " name " 前后加空格的方式匹配（行首是空格、名字前面还有对齐空格）。
        let has = |name: &str| {
            available.lines().any(|l| {
                l.split_whitespace()
                    .nth(1)
                    .map(|w| w == name)
                    .unwrap_or(false)
            })
        };

        let picked: Option<&'static str> = if has("libx264") {
            Some("libx264")
        } else if has("libopenh264") {
            Some("libopenh264")
        } else {
            None
        };
        match picked {
            Some(name) => log::info!("使用 H.264 编码器: {}", name),
            None => log::warn!("未找到可用的 H.264 编码器，视频转码将失败"),
        }
        picked
    })
}

/// ffmpeg 可执行文件路径（允许用环境变量覆盖）
fn ffmpeg_bin() -> String {
    std::env::var("FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".to_string())
}

pub fn status() -> QueueStatus {
    let st = state();
    let guard = st.lock().unwrap_or_else(|e| e.into_inner());
    QueueStatus {
        enabled: ffmpeg_available(),
        pending: guard.pending.len(),
        running: guard.worker_running,
    }
}

/// 把原始视频加入转码队列。
///
/// 输入 → 输出：原始视频的绝对路径 → 立即返回（不等待转码）
/// 不变量：同一路径在排队期间重复调用只会入队一次
/// 边界：ffmpeg 不可用时直接返回，不产生任何副作用
pub fn enqueue(raw: PathBuf) {
    if !ffmpeg_available() {
        return;
    }
    let st = state();

    {
        let mut guard = st.lock().unwrap_or_else(|e| e.into_inner());
        let key = raw.to_string_lossy().to_string();
        // 已经在排队/处理中的跳过（幂等入队）
        if guard.known.contains(&key) {
            return;
        }
        // 已经转码过的不重复转（比如服务重启后重新扫描）
        let output = transcoded_path(&raw);
        if output.exists() {
            return;
        }
        guard.known.insert(key);
        guard.pending.push(raw.clone());

        if guard.worker_running {
            log::info!(
                "转码队列 +1（待处理 {}）: {}",
                guard.pending.len(),
                raw.display()
            );
            return;
        }
        guard.worker_running = true;
    }

    // 第一个任务：拉起唯一的 worker
    let st2 = st.clone();
    std::thread::spawn(move || worker_loop(st2));
}

/// 转码后的文件名：`xxx.mp4` → `xxx.transcoded.mp4`，封面 `xxx.poster.jpg`
fn transcoded_path(raw: &Path) -> PathBuf {
    let stem = raw.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    raw.with_file_name(format!("{}.transcoded.mp4", stem))
}

pub fn poster_path(raw: &Path) -> PathBuf {
    let stem = raw.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    raw.with_file_name(format!("{}.poster.jpg", stem))
}

/// 给前端用的转码产物 URL（没有转码产物时返回 None）。
///
/// 输入 → 输出：原始文件名 → `/static/uploads/<name>.transcoded.mp4`
/// 关键机制：按命名约定推导路径并检查文件是否存在
/// 边界：传入的如果已经是 `.transcoded.mp4`，不会再叠加后缀
pub fn transcoded_url(filename: &str) -> Option<String> {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())?
        .trim_end_matches(".transcoded");
    let out = paths::uploads_dir().join(format!("{}.transcoded.mp4", stem));
    if out.is_file() {
        Some(format!("/static/uploads/{}.transcoded.mp4", stem))
    } else {
        None
    }
}

fn worker_loop(st: Arc<Mutex<QueueState>>) {
    loop {
        let next = {
            let mut guard = st.lock().unwrap_or_else(|e| e.into_inner());
            // FIFO：先入队的先转，避免大文件把小文件饿死
            if guard.pending.is_empty() {
                guard.worker_running = false;
                return;
            }
            guard.pending.remove(0)
        };

        let started = std::time::Instant::now();
        match transcode(&next) {
            Ok(()) => log::info!(
                "转码完成 {}（耗时 {}s）",
                next.display(),
                started.elapsed().as_secs()
            ),
            Err(e) => log::warn!("转码失败 {}: {}", next.display(), e),
        }

        // 无论成败都移出 in-flight 集合，允许将来重试
        let mut guard = st.lock().unwrap_or_else(|e| e.into_inner());
        guard.known.remove(&next.to_string_lossy().to_string());
    }
}

/// 真正调用 ffmpeg
///
/// 输入 → 输出：原始视频 → (转码 mp4, 封面 jpg)，失败返回错误字符串
/// 关键机制：
///   - `-vf scale='min(1280,iw)':-2` 只缩小不放大，且保证偶数高度（H.264 要求）
///   - `-movflags +faststart` 把 moov 移到文件头，浏览器可以边下边播
///   - 优先 copy 音轨（不重编码音频），失败再转 AAC——省一半时间
///   - 单独抽一帧封面，转码失败也尽量有封面
/// 边界：源文件分辨率已小于 720p 时仍然重编码（音视频元数据更干净），
///       只是不够省时间；这是用时间换"一定能播"的取舍
fn transcode(raw: &Path) -> Result<(), String> {
    let output = transcoded_path(raw);
    let poster = poster_path(raw);

    let Some(encoder) = pick_h264_encoder() else {
        return Err("没有可用的 H.264 编码器".to_string());
    };

    // 两个编码器的参数不通用：x264 用 -preset/-crf，openh264 只认码率
    let video_opts: Vec<String> = if encoder == "libx264" {
        vec![
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "veryfast".into(),
            "-crf".into(),
            "23".into(),
        ]
    } else {
        vec![
            "-c:v".into(),
            "libopenh264".into(),
            "-b:v".into(),
            "2000k".into(),
        ]
    };

    // 封面：失败不影响主流程
    let _ = run_ffmpeg(&[
        "-y".into(),
        "-ss".into(),
        "0.5".into(),
        "-i".into(),
        raw.display().to_string(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        format!("scale='min({},iw)':-2", MAX_LONG_EDGE),
        "-q:v".into(),
        "4".into(),
        poster.display().to_string(),
    ]);

    let scale = format!("scale='min({},iw)':-2", MAX_LONG_EDGE);

    let mut audio_copy: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        raw.display().to_string(),
        "-vf".into(),
        scale.clone(),
    ];
    audio_copy.extend(video_opts.clone());
    audio_copy.extend(vec![
        "-c:a".into(),
        "copy".into(),
        "-movflags".into(),
        "+faststart".into(),
        output.display().to_string(),
    ]);

    if run_ffmpeg(&audio_copy).is_ok() {
        return Ok(());
    }

    // 兜底：音频也重编码
    let mut full_reencode: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        raw.display().to_string(),
        "-vf".into(),
        scale,
    ];
    full_reencode.extend(video_opts);
    full_reencode.extend(vec![
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "128k".into(),
        "-movflags".into(),
        "+faststart".into(),
        output.display().to_string(),
    ]);
    run_ffmpeg(&full_reencode).map(|_| ())
}

fn run_ffmpeg(args: &[String]) -> Result<(), String> {
    log::info!("ffmpeg {}", args.join(" "));
    let mut child = std::process::Command::new(ffmpeg_bin())
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // 转码是子进程；服务重启时不希望它成为孤儿
        .spawn()
        .map_err(|e| format!("启动 ffmpeg 失败: {}", e))?;

    let mut stderr = child.stderr.take();

    // 简单超时：轮询退出状态，超过上限就杀掉
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let mut err = String::new();
                if let Some(s) = stderr.as_mut() {
                    use std::io::Read;
                    let _ = s.read_to_string(&mut err);
                }
                let tail: String = err.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
                return Err(format!("ffmpeg 退出码 {:?}: {}", status.code(), tail));
            }
            Ok(None) => {
                if started.elapsed() > TRANSCODE_TIMEOUT {
                    let _ = child.kill();
                    return Err("转码超时".to_string());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("等待 ffmpeg 失败: {}", e)),
        }
    }
}
