//! 登录失败限速（进程内，按「用户名 + 客户端 IP」双维度计数）。
//!
//! 为什么需要：
//!   alpha 前没有失败锁定，`POST /api/login` 可以被无成本地反复尝试；
//!   密码最短只要求 6 位，在线暴力破解是可执行的。
//!
//! 输入 → 输出：(动作, 用户名, 客户端 IP) → 是否允许 / 剩余等待秒数
//! 关键机制：
//!   - 两个独立计数器：按用户名（防止只盯一个人），按 IP（防止换着用户名撞字典）
//!   - 冷却窗口 `WINDOW`：窗口内累计失败达到上限即锁定 `LOCK` 秒
//!   - 成功登录清零用户名计数；IP 计数保留（成功的人也可能在试别人的账号）
//!   - 每分钟清理一次过期条目，内存不随攻击次数增长
//! 不变量：被锁定时 `check` 返回 Err，handler 在任何耗时操作（bcrypt 校验）之前就返回
//! 边界：
//!   - 进程内内存表：多实例部署不共享（本项目单机单进程，不涉及）
//!   - 反向代理后 `peer_addr` 是 127.0.0.1，真实 IP 取自 X-Forwarded-For 的首段；
//!     该头可被伪造，因此它只用来「增加区分度」，不承担安全边界
//!   - 重启进程即清空（可接受：攻击者也要重新建立节奏）
//!
//! 依据：
//!   经典问题：限流 / 令牌桶（rate limiting）与账户锁定（account lockout）
//!   本处变体：单机无 Redis，状态只能放进程内存；且要同时防「猜单个账号」与「撞库」
//!   沿用结论：滑动窗口 + 固定上限足以把在线爆破从「可行」压到「不划算」
//!   偏离处：不做指数退避，锁定时间是固定的；代价是误锁恢复慢，
//!           但个人站流量小，固定 15 分钟比复杂退避更可预期

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use actix_web::HttpRequest;

use crate::errors::AppError;

/// 统计窗口：在这个时间窗内累计的失败才计入
const WINDOW: Duration = Duration::from_secs(15 * 60);
/// 同一维度失败多少次后锁定
const MAX_FAILS: u32 = 5;
/// 锁定时长
const LOCK: Duration = Duration::from_secs(15 * 60);
/// 清理过期条目的间隔
const SWEEP_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
struct Entry {
    fails: u32,
    first_fail: Instant,
    locked_until: Option<Instant>,
}

impl Entry {
    fn new() -> Self {
        Entry {
            fails: 0,
            first_fail: Instant::now(),
            locked_until: None,
        }
    }

    fn expired(&self, now: Instant) -> bool {
        self.locked_until.map(|t| t > now).unwrap_or(false) == false
            && now.duration_since(self.first_fail) > WINDOW
    }

    fn retry_after(&self, now: Instant) -> Option<u64> {
        match self.locked_until {
            Some(t) if t > now => Some(t.duration_since(now).as_secs().max(1)),
            _ => None,
        }
    }
}

struct State {
    by_user: HashMap<String, Entry>,
    by_ip: HashMap<String, Entry>,
    last_sweep: Instant,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            by_user: HashMap::new(),
            by_ip: HashMap::new(),
            last_sweep: Instant::now(),
        })
    })
}

/// 取客户端 IP：优先 X-Forwarded-For 首段（Nginx 会写入），退回 socket 对端地址
fn client_ip(req: &HttpRequest) -> String {
    if let Some(v) = req.headers().get("X-Forwarded-For") {
        if let Ok(s) = v.to_str() {
            if let Some(first) = s.split(',').next() {
                let t = first.trim();
                if !t.is_empty() {
                    // 只接受合法 IP，避免把任意字符串灌进内存表
                    if t.parse::<IpAddr>().is_ok() {
                        return t.to_string();
                    }
                }
            }
        }
    }
    match req.peer_addr() {
        Some(addr) => addr.ip().to_string(),
        None => "unknown".to_string(),
    }
}

fn sweep(s: &mut State, now: Instant) {
    if now.duration_since(s.last_sweep) < SWEEP_EVERY {
        return;
    }
    s.last_sweep = now;
    s.by_user.retain(|_, e| !e.expired(now));
    s.by_ip.retain(|_, e| !e.expired(now));
}

/// 登录前调用：任一维度被锁即返回 429，并带剩余秒数
pub fn check(req: &HttpRequest, username: &str) -> Result<(), AppError> {
    let user_key = username.trim().to_lowercase();
    let ip = client_ip(req);
    let now = Instant::now();
    let mut s = state().lock().unwrap_or_else(|e| e.into_inner());
    sweep(&mut s, now);

    for (map, key) in [(&s.by_user, &user_key), (&s.by_ip, &ip)] {
        if let Some(entry) = map.get(key) {
            if let Some(secs) = entry.retry_after(now) {
                let mins = secs.div_ceil(60);
                return Err(AppError::TooManyRequests(format!(
                    "尝试次数过多，请约 {} 分钟后再试",
                    mins
                )));
            }
        }
    }
    Ok(())
}

/// 登录失败后调用：两个维度同时 +1，达到上限即锁定
pub fn record_failure(req: &HttpRequest, username: &str) {
    let user_key = username.trim().to_lowercase();
    let ip = client_ip(req);
    let now = Instant::now();
    let mut s = state().lock().unwrap_or_else(|e| e.into_inner());

    bump(&mut s.by_user, user_key, now);
    bump(&mut s.by_ip, ip, now);
}

/// 单个维度计数 +1，达到上限即锁定
fn bump(map: &mut HashMap<String, Entry>, key: String, now: Instant) {
    let entry = map.entry(key).or_insert_with(Entry::new);
    // 窗口已过：重新开始计数
    if entry.fails > 0 && now.duration_since(entry.first_fail) > WINDOW {
        *entry = Entry::new();
    }
    entry.fails += 1;
    if entry.fails == 1 {
        entry.first_fail = now;
    }
    if entry.fails >= MAX_FAILS {
        entry.locked_until = Some(now + LOCK);
        log::warn!("登录限速：某维度连续失败 {} 次，锁定 {:?}", entry.fails, LOCK);
    }
}

/// 登录成功后调用：清零该用户名的失败计数
pub fn record_success(username: &str) {
    let user_key = username.trim().to_lowercase();
    let mut s = state().lock().unwrap_or_else(|e| e.into_inner());
    s.by_user.remove(&user_key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_locks_after_max_fails() {
        let now = Instant::now();
        let mut e = Entry::new();
        e.fails = MAX_FAILS;
        e.locked_until = Some(now + LOCK);
        assert!(e.retry_after(now).is_some());
        assert!(e.retry_after(now + LOCK + Duration::from_secs(1)).is_none());
    }

    #[test]
    fn entry_expires_after_window() {
        let now = Instant::now();
        let mut e = Entry::new();
        e.fails = 3;
        e.first_fail = now - WINDOW - Duration::from_secs(1);
        assert!(e.expired(now));
    }
}
