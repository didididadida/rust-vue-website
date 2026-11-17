//! 服务层
//!
//! 逻辑描述：
//!   输入 → 输出：handlers 层的业务调用 → 具体存储操作（数据库 / 文件）
//!   关键机制：所有函数接收 `&DatabaseConnection`，由 handlers 从 `web::Data` 取出传入
//!   不变量：
//!     - 服务层不关心 HTTP（不出现 HttpRequest / HttpResponse）
//!     - handlers 层不做业务判断（不在 handler 里拼 SQL）
//!   边界：上传文件的字节流仍由 handlers 层接收，服务层只管元数据
//!
//! 依据：
//!   经典问题：分层架构中的关注点分离（separation of concerns）
//!   来源：
//!     1. 本机全局约定「层级纪律」第 3 节「代码同样要有层级」：
//!        服务层放业务逻辑与校验，不许放 SQL 拼语句、HTTP 细节。
//!     2. actix 官方示例的布局（handlers 只做参数提取与服务调用）
//!        https://github.com/snowdreamtech/template/blob/main/.agent/rules/actix-web.md
//!   原型结论：
//!     1. 把 SQL 与业务判断混在 handler 里，会导致无法复用与难以测试；
//!     2. 服务层无 HTTP 依赖后，可以被定时任务、CLI 脚本直接调用。
//!   本处变体：原实现把文件读写直接放在 handlers 里（一个文件几百行）；
//!             引入 ORM 后数据库操作变多，必须分层。
//!   沿用结论：新增 services 层，handlers 只做参数解析与调用。
//!   偏离处：没有引入 trait 抽象（如 `trait UserRepo`）。
//!           本项目只有一种存储实现（SQLite），做接口抽象是过度设计。

pub mod article;
pub mod comment;
pub mod forum;
pub mod showcase;
pub mod message;
pub mod profile;
pub mod upload;
pub mod user;
