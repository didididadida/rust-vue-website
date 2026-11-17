use serde::{Deserialize, Serialize};

// ==================== 请求模型 ====================

/// 新增/更新文章（PUT 为整体替换，缺省字段按空值处理）
#[derive(Debug, Deserialize)]
pub struct ArticleUpsert {
    pub title: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub cover_image: Option<String>,
    #[serde(default)]
    pub is_published: Option<bool>,
    #[serde(default)]
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

// ==================== 注册与个人主页 ====================

/// 发送邮箱验证码
#[derive(Debug, Deserialize)]
pub struct SendCodeRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

/// 提交验证码完成注册
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub code: String,
}

/// 修改个人资料
#[derive(Debug, Deserialize)]
pub struct ProfileUpdate {
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    /// 个人主页头图（B 站空间页顶部那张大图），留空则用主题渐变
    #[serde(default)]
    pub banner: Option<String>,
    /// 头像裁剪：缩放倍数与偏移百分比（省略表示不改）
    #[serde(default)]
    pub avatar_zoom: Option<f32>,
    #[serde(default)]
    pub avatar_x: Option<i16>,
    #[serde(default)]
    pub avatar_y: Option<i16>,
    #[serde(default)]
    pub email: Option<String>,
}

/// 修改密码
#[derive(Debug, Deserialize)]
pub struct PasswordChange {
    pub old_password: String,
    pub new_password: String,
}

/// 个人主页数据（公开）
#[derive(Debug, Serialize)]
pub struct ProfileView {
    pub username: String,
    pub nickname: String,
    pub bio: String,
    pub avatar: String,
    /// 个人主页头图 URL；为空表示用主题渐变
    #[serde(default)]
    pub banner: String,
    /// 头像裁剪：缩放倍数
    #[serde(default)]
    pub avatar_zoom: f32,
    /// 头像裁剪：水平/垂直偏移（百分比）
    #[serde(default)]
    pub avatar_x: i16,
    #[serde(default)]
    pub avatar_y: i16,
    pub is_admin: bool,
    pub joined_at: String,
    /// 站内头衔，本站所有注册用户的头衔都是「船长」
    pub title: String,
    /// 发表过的评论数量
    pub comment_count: usize,
    /// 最近评论（最多 10 条），带所属文章 slug 与标题
    pub recent_comments: Vec<ProfileComment>,
}

#[derive(Debug, Serialize)]
pub struct ProfileComment {
    pub slug: String,
    pub article_title: String,
    pub content: String,
    pub created_at: String,
}

/// 登录后返回的当前用户信息（含资料，供前端展示）
#[derive(Debug, Serialize)]
pub struct MeDetail {
    pub username: String,
    pub nickname: String,
    pub bio: String,
    pub avatar: String,
    /// 个人主页头图 URL；为空表示用主题渐变
    #[serde(default)]
    pub banner: String,
    /// 头像裁剪：缩放倍数
    #[serde(default)]
    pub avatar_zoom: f32,
    /// 头像裁剪：水平/垂直偏移（百分比）
    #[serde(default)]
    pub avatar_x: i16,
    #[serde(default)]
    pub avatar_y: i16,
    pub email: String,
    pub is_admin: bool,
}

/// 发表评论的请求体
#[derive(Debug, Deserialize)]
pub struct CommentUpsert {
    /// 昵称
    pub author: String,
    /// 正文（纯文本，前端按纯文本渲染，不做 HTML 注入）
    pub content: String,
    /// 邮箱（可选，仅管理员可见）
    #[serde(default)]
    pub email: Option<String>,
    /// 回复的父评论 id（可选）
    #[serde(default)]
    pub parent_id: Option<String>,
    /// 蜜罐字段：正常用户看不到、不会填；填了就当作机器人丢弃
    #[serde(default)]
    pub website: Option<String>,
}

/// 审核评论的请求体
#[derive(Debug, Deserialize)]
pub struct CommentApprove {
    pub approved: bool,
}

// ==================== 响应模型 ====================

#[derive(Debug, Serialize)]
pub struct ArticleView {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub cover_image: Option<String>,
    pub is_published: bool,
    /// 作者用户名；本站作者为空
    pub author: Option<String>,
    /// 审核状态：pending / approved / rejected
    pub review_status: String,
    /// 是否首页可见
    pub is_featured: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub username: String,
    pub is_admin: bool,
    /// 展示名（昵称，未设置时等于用户名）
    pub nickname: String,
    /// 头像 URL，可能为空
    pub avatar: String,
}

/// 公开可见的评论（不含邮箱）
#[derive(Debug, Serialize)]
pub struct CommentView {
    pub id: String,
    pub parent_id: Option<String>,
    pub author: String,
    pub content: String,
    pub is_approved: bool,
    pub created_at: String,
}

/// 管理员可见的评论（含邮箱与所属文章）
#[derive(Debug, Serialize)]
pub struct CommentAdminView {
    pub slug: String,
    pub id: String,
    pub parent_id: Option<String>,
    pub author: String,
    pub email: String,
    pub content: String,
    pub is_approved: bool,
    pub created_at: String,
}

/// 评论列表响应：总条数 + 列表
#[derive(Debug, Serialize)]
pub struct CommentListResponse {
    pub total: usize,
    pub items: Vec<CommentView>,
}

#[derive(Debug, Serialize)]
pub struct ImageInfo {
    pub filename: String,
    pub url: String,
    pub size: u64,
    pub size_human: String,
    pub modified_at: String,
    /// 上传者用户名；历史图片（归属表没有记录）为空串
    pub uploader: String,
    /// 当前登录用户是否可以删除这张图（站长对所有图都有权限）
    pub can_delete: bool,
    /// 视频的转码产物地址（720p mp4）；非视频或尚未转码时为 null
    pub transcoded_url: Option<String>,
    /// 是否为视频文件（前端据此决定用 <video> 还是 <img> 渲染）
    #[serde(default)]
    pub is_video: bool,
    /// 视频首帧封面地址（`xxx.poster.jpg`）；非视频或封面未生成时为 null
    #[serde(default)]
    pub poster_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub version: String,
    pub storage: StorageResponse,
}

#[derive(Debug, Serialize)]
pub struct StorageResponse {
    pub root: String,
    pub articles: usize,
    pub comments: usize,
}

/// 列表视图的公共字段
pub trait Dated {
    fn created_at(&self) -> &str;
    fn slug(&self) -> &str;
}

impl Dated for ArticleView {
    fn created_at(&self) -> &str {
        &self.created_at
    }
    fn slug(&self) -> &str {
        &self.slug
    }
}

// ==================== 列表输出（可选剔除正文）====================

/// 列表响应：`fields=meta` 时剔除正文，`fields=full`（或显式要正文）时原样输出
///
/// 输入 → 输出：完整视图列表 + 是否需要正文 → 序列化后的 JSON 数组
/// 关键机制：用一个 Enum 在序列化层做"列投影"，调用方无需改动业务结构
/// 不变量：两种分支下的 JSON 字段名集合完全相同（`content` 恒存在，meta 时为空串），
///   前端不需要按分支写两套解构
/// 边界：空串 content 与"真的空正文"从 JSON 上不可区分——列表页本就不该依赖它
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ListOut<T> {
    Full(Vec<T>),
    Meta(Vec<T>),
}

/// 按 `with_content` 决定走 Full 还是 Meta 分支
pub fn list_out<T, F>(items: Vec<T>, with_content: bool, mut strip: F) -> ListOut<T>
where
    F: FnMut(&T) -> T,
{
    if with_content {
        ListOut::Full(items)
    } else {
        // 用 drain 而不是 iter + collect：避免再分配一次 Vec<T>
        ListOut::Meta(items.into_iter().map(|item| strip(&item)).collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub is_admin: bool,
}

impl ArticleView {
    /// 去掉正文的列表视图：字段名集合不变，content 置为空串
    ///
    /// 不变量：除 `content` 外所有字段与原件逐位相同
    /// 边界：空串与"真的空正文"从 JSON 上不可区分——列表页本就不该依赖正文
    pub fn without_content(&self) -> Self {
        ArticleView {
            slug: self.slug.clone(),
            title: self.title.clone(),
            content: String::new(),
            summary: self.summary.clone(),
            category: self.category.clone(),
            tags: self.tags.clone(),
            cover_image: self.cover_image.clone(),
            is_published: self.is_published,
            author: self.author.clone(),
            review_status: self.review_status.clone(),
            is_featured: self.is_featured,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

// ==================== 列表通用逻辑 ====================

/// 列表是否需要正文
///
/// 输入 → 输出：?fields= 的原始值 → 是否保留正文
/// 关键机制：只有显式传 `full` 才返回正文，其余（缺省、空串、未知值）一律按
///   `meta` 处理。默认"瘦"而不是默认"胖"——列表页永远比详情页多，浪费的量级更大
/// 不变量：不需要正文时，列表响应体积与正文长度无关
/// 边界：老前端/第三方调用方若直接依赖列表里的 content，会拿到空串；
///   站内所有列表页都已核对过，只用 summary/description，不读 content
///
/// 依据：
///   经典问题：查询投影（projection / API 字段裁剪）+ 默认值的最小惊讶原则
///   本处变体：没有数据库可以在 SQL 层 SELECT 列，记录本来就是整文件读入内存，
///             所以裁剪发生在"序列化出口"而不是"读取入口"
///   沿用结论：把传输量降到与返回条数成正比、与正文长度无关，与 SELECT 列投影同效
///   偏离处：读取侧仍有全量正文的内存开销（Markdown 文件本来就要整体解析），
///           只省了 JSON 序列化与网络传输 —— 对手机端网络瓶颈是主要收益
pub fn wants_content(fields: Option<&str>) -> bool {
    fields
        .map(|f| f.trim().eq_ignore_ascii_case("full"))
        .unwrap_or(false)
}

/// 列表项按创建时间倒序（同时间按 slug 稳定排序）
pub fn sort_newest_first<T: Dated>(items: &mut [T]) {
    items.sort_by(|a, b| {
        b.created_at()
            .cmp(a.created_at())
            .then_with(|| a.slug().cmp(b.slug()))
    });
}

/// 分页：limit 限定在 1..=200
pub fn paginate<T>(items: Vec<T>, skip: i64, limit: Option<i64>) -> Vec<T> {
    let skip = skip.max(0) as usize;
    let limit = limit.unwrap_or(50).clamp(1, 200) as usize;
    items.into_iter().skip(skip).take(limit).collect()
}

/// 归一化 category 查询参数（空值 / all 视为不过滤）
pub fn active_category(category: Option<&str>) -> Option<&str> {
    category
        .map(str::trim)
        .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("all"))
}

/// 分类比较（忽略大小写）
pub fn matches_category(item: &Option<String>, want: &str) -> bool {
    item.as_deref()
        .map(|c| c.eq_ignore_ascii_case(want))
        .unwrap_or(false)
}

/// 统一响应结构：`{ "code": 0, "message": "success", "data": ... }`
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        ApiResponse {
            code: 0,
            message: "success".to_string(),
            data: Some(data),
        }
    }

    pub fn error(code: i32, message: &str) -> Self {
        ApiResponse {
            code,
            message: message.to_string(),
            data: None,
        }
    }
}
