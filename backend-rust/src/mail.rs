//! 邮件发送：通过 SMTP 发出验证码。
//!
//! 配置（写在 `.env`）：
//!
//! ```text
//! SMTP_HOST=smtp.qq.com
//! SMTP_PORT=465
//! SMTP_USER=your@qq.com
//! SMTP_PASS=授权码            # 注意：是授权码不是登录密码
//! SMTP_FROM=wanle <your@qq.com>
//! ```
//!
//! 没配置 SMTP 时的行为由 `MAIL_MODE` 决定：
//!
//! - `log`（默认）：不真发信，把验证码打到服务日志。
//!   适合本地开发 —— 你能在 `journalctl` 里看到验证码并完成注册流程。
//! - `off`：直接报错，表示"邮件功能未开启"。
//!
//! 依据：
//!   经典问题：外部依赖的可降级适配（graceful degradation）
//!   本处变体：核心业务（注册）依赖外部服务（SMTP），但开发期不该被它卡住
//!   沿用结论：把"发送"抽象成一个可能失败的副作用，失败时明确报错而非静默吞掉
//!   偏离处：日志模式下验证码进入日志文件，生产环境必须显式配置 SMTP

use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub from: String,
}

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// 读取 SMTP 配置；缺任何一项都返回 None
pub fn config() -> Option<SmtpConfig> {
    let host = env_opt("SMTP_HOST")?;
    let user = env_opt("SMTP_USER")?;
    let pass = env_opt("SMTP_PASS")?;
    let port = env_opt("SMTP_PORT")
        .and_then(|v| v.parse().ok())
        .unwrap_or(465u16);
    // 发件人默认用登录账号
    let from = env_opt("SMTP_FROM").unwrap_or_else(|| format!("wanle <{}>", user));
    Some(SmtpConfig {
        host,
        port,
        user,
        pass,
        from,
    })
}

/// 邮件模式：smtp（真发） / log（打到日志） / off
fn mail_mode() -> String {
    env_opt("MAIL_MODE")
        .map(|v| v.trim().to_lowercase())
        .unwrap_or_else(|| {
            if config().is_some() {
                "smtp".into()
            } else {
                "log".into()
            }
        })
}

fn body_text(code: &str, ttl_min: i64) -> String {
    format!(
        "欢迎注册「wanle」。\n\n\
         你的验证码是：{code}\n\n\
         验证码 {ttl_min} 分钟内有效，请勿转发给他人。\n\
         如果这不是你本人的操作，忽略本邮件即可。\n\n\
         —— wanle",
        code = code,
        ttl_min = ttl_min
    )
}

fn body_html(code: &str, ttl_min: i64) -> String {
    // 邮件客户端对 CSS 支持有限，这里全用内联样式
    format!(
        r#"<!DOCTYPE html>
<html><body style="margin:0;padding:32px 16px;background:#05070f;font-family:-apple-system,'PingFang SC','Microsoft YaHei',sans-serif;">
  <div style="max-width:460px;margin:0 auto;background:#0a0f1d;border:1px solid rgba(126,156,210,0.18);border-radius:12px;padding:32px;">
    <p style="margin:0 0 6px;color:#4cc9f0;font-size:12px;letter-spacing:3px;">CIJIU&nbsp;STATION</p>
    <h1 style="margin:0 0 18px;color:#eaf2ff;font-size:19px;font-weight:600;">验证你的邮箱</h1>
    <p style="margin:0 0 20px;color:#93a3bd;font-size:14px;line-height:1.7;">
      你正在注册「wanle」，请在页面输入下面的验证码完成验证。
    </p>
    <div style="background:#060a14;border:1px solid rgba(76,201,240,0.28);border-radius:10px;padding:20px;text-align:center;">
      <div style="color:#7ddaf7;font-size:32px;font-weight:700;letter-spacing:10px;font-family:ui-monospace,Consolas,monospace;">{code}</div>
    </div>
    <p style="margin:20px 0 0;color:#5d6c87;font-size:13px;line-height:1.7;">
      验证码 {ttl_min} 分钟内有效，请勿转发给他人。<br>
      如果这不是你本人的操作，忽略本邮件即可。
    </p>
  </div>
</body></html>"#,
        code = code,
        ttl_min = ttl_min
    )
}

/// 发送验证码邮件
pub async fn send_code(to: &str, code: &str, ttl_min: i64) -> Result<(), AppError> {
    match mail_mode().as_str() {
        "off" => Err(AppError::Internal(
            "邮件服务未开启，请联系管理员".to_string(),
        )),
        "log" => {
            log::warn!("======================================================");
            log::warn!(" [MAIL_MODE=log] 未配置 SMTP，验证码打在日志里：");
            log::warn!("   收件人: {}", to);
            log::warn!("   验证码: {}", code);
            log::warn!("   有效期: {} 分钟", ttl_min);
            log::warn!("======================================================");
            Ok(())
        }
        _ => {
            let cfg = config().ok_or_else(|| {
                AppError::Internal("SMTP 配置不完整（需要 SMTP_HOST/USER/PASS）".to_string())
            })?;
            send_via_smtp(&cfg, to, code, ttl_min).await
        }
    }
}

async fn send_via_smtp(
    cfg: &SmtpConfig,
    to: &str,
    code: &str,
    ttl_min: i64,
) -> Result<(), AppError> {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let from_addr = extract_addr(&cfg.from);
    let email = Message::builder()
        .from(
            cfg.from
                .parse()
                .map_err(|e| AppError::Internal(format!("发件人格式错误: {}", e)))?,
        )
        .to(to
            .parse()
            .map_err(|e| AppError::BadRequest(format!("收件人邮箱格式错误: {}", e)))?)
        .subject("【wanle】邮箱验证码")
        .multipart(lettre::message::MultiPart::alternative_plain_html(
            body_text(code, ttl_min),
            body_html(code, ttl_min),
        ))
        .map_err(|e| AppError::Internal(format!("构造邮件失败: {}", e)))?;

    let _ = (ContentType::TEXT_HTML, from_addr);

    // 465 走隐式 TLS，587 走 STARTTLS
    let builder = if cfg.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
    }
    .map_err(|e| AppError::Internal(format!("SMTP 连接初始化失败: {}", e)))?;

    let mailer = builder
        .port(cfg.port)
        .credentials(Credentials::new(cfg.user.clone(), cfg.pass.clone()))
        .build();

    mailer
        .send(email)
        .await
        .map_err(|e| AppError::Internal(format!("发送邮件失败: {}", e)))?;

    log::info!("验证码邮件已发送至 {}", to);
    Ok(())
}

/// 从 `名字 <邮箱>` 里取出邮箱部分
fn extract_addr(s: &str) -> String {
    if let (Some(a), Some(b)) = (s.find('<'), s.find('>')) {
        if a < b {
            return s[a + 1..b].trim().to_string();
        }
    }
    s.trim().to_string()
}
