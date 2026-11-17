//! 邮箱验证码实体
//!
//! 逻辑描述：
//!   输入 → 输出：邮箱 + 验证码哈希 + 过期时间 ↔ `verifications` 表的一行
//!   关键机制：只存验证码的 SHA-256 哈希，不存明文；
//!             同一邮箱重复请求时覆盖旧记录（INSERT OR REPLACE 语义）
//!   不变量：表中永不出现明文验证码
//!   边界：过期记录由定时清理或读取时判定，不自动删除
//!
//! 依据：
//!   经典问题：一次性凭证的短期存储（one-time credential）
//!   来源：OWASP Authentication Cheat Sheet 关于一次性验证码的条目
//!        https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
//!        以及 HMAC/哈希存储口令类材料的通行做法
//!   原型结论：
//!     1. 验证码属于短期凭证，存储必须防泄露（存哈希即可，无需可逆）；
//!     2. 有效期 + 重发间隔是最小防护（VERIFY_TTL_MINUTES / VERIFY_RESEND_SECONDS）。
//!   本处变体：原实现把验证码存在进程内存的 HashMap 里，
//!             重启即失效——迁到表后重启仍可用，需要显式过期判定。
//!   沿用结论：只存 sha256(code)，比对时对用户输入做同样哈希
//!   偏离处：原先内存方案不需要清理，入库后增加"读取时判断过期"以避免残留

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "verifications")]
pub struct Model {
    /// 邮箱作为主键：同一邮箱只保留最新一条验证码
    #[sea_orm(primary_key, auto_increment = false)]
    pub email: String,

    /// 验证码的 SHA-256 哈希（十六进制）
    pub code_hash: String,

    /// 过期时间（RFC3339）
    pub expires_at: String,

    /// 最近一次发送时间（RFC3339），用于计算重发间隔
    pub sent_at: String,
}

impl ActiveModelBehavior for ActiveModel {}
