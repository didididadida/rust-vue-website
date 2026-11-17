//! 用户实体
//!
//! 逻辑描述：
//!   输入 → 输出：账号字段 ↔ `users` 表的一行
//!   关键机制：密码只存 bcrypt 哈希；username 唯一且作为对外标识
//!   不变量：`password_hash` 永远是哈希，绝不出现明文
//!   边界：用户被删时其资料、验证码由服务层清理
//!
//! 依据：
//!   经典问题：口令的不可逆存储（password hashing）
//!   来源：OWASP Password Storage Cheat Sheet
//!        https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
//!        结论：使用自适应哈希（bcrypt/scrypt/Argon2），每用户独立 salt
//!   原型结论：bcrypt 自带 salt，cost ≥ 10；绝不存明文或可逆加密
//!   本处变体：原本存在 `users.txt`（一行一个用户，冒号分隔），
//!             迁移时哈希字符串原样搬过去，不重新计算。
//!   沿用结论：password_hash 列直接存 bcrypt 串（含算法前缀与 cost）
//!   偏离处：未加 last_login / 失败计数等列——失败计数在内存里做（ratelimit）

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub username: String,

    /// bcrypt 哈希串，含算法前缀与 cost
    pub password_hash: String,

    pub is_admin: bool,

    /// 邮箱，可为空（历史账号）
    #[sea_orm(indexed)]
    pub email: String,

    /// 邮箱是否已验证
    pub verified: bool,

    /// 展示昵称，为空时回退到用户名
    pub nickname: String,

    /// 该用户的个人资料（一对一）
    #[sea_orm(has_one)]
    pub profile: HasOne<super::profile::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 对外展示的名字：优先昵称，为空则回退用户名
    pub fn display_name(&self) -> &str {
        if self.nickname.trim().is_empty() {
            &self.username
        } else {
            &self.nickname
        }
    }
}
