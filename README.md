# Rust+Vue 网站项目

用 **Rust + actix-web** 做后端、**Vue 3 + Vite** 做前端的个人博客。
没有数据库 ORM，没有 UI 组件库。文章存 Markdown 文件，上传记录存 SQLite。

## 仓库内容

```
backend-rust/        Rust 后端（actix-web）
  src/
    main.rs          入口：初始化目录、密钥、管理员账号，注册路由
    paths.rs         所有存储路径（支持环境变量覆盖）
    content/         Markdown + front matter 的读写与解析
    db.rs            图床元数据库（SQLite）
    comments.rs      评论存储
    users.rs         用户表读写
    profile.rs       个人资料
    verification.rs  邮箱验证码（只存令牌哈希）
    ratelimit.rs     登录失败限速
    images.rs        图片无损重编码
    transcode.rs     视频转码队列
    mail.rs          SMTP 邮件发送
    handlers/        REST 接口
      articles.rs    文章
      comments.rs    评论
      upload.rs      图床
      auth.rs        登录
      register.rs    注册与资料
      health.rs      健康检查
      mod.rs         路由注册
    middleware/      JWT 签发与校验
  Cargo.toml
frontend/            Vue 3 前端
  src/
    views/           9 个页面
      Home.vue       首页
      Blog.vue       文章列表
      BlogDetail.vue 文章详情
      Admin.vue      管理后台
      ImageBed.vue   图床
      Login.vue / Register.vue
      Profile.vue    个人主页
      NotFound.vue
    components/      7 个组件
      RichEditor.vue     B 站式编辑器（评论用）
      MarkdownEditor.vue 全屏 Markdown 编辑器
      DataTable.vue      后台表格
      CommentSection.vue 评论区
      DetailLayout.vue   详情页布局
      UserAvatar.vue     统一头像
      AvatarCropper.vue  头像裁剪
    api.js           axios 实例（自动带 token、拆包）
    auth.js          登录状态
    markdown.js      Markdown 渲染 + 代码高亮
    labels.js        分类名、日期格式化
    time.js          相对时间、头像配色
    emoji.js         表情数据
  index.html
  package.json
  vite.config.js
nginx/nginx.conf    生产环境反代配置
docs/
  index.html        GitHub Pages 展示页
  运维手册.md        部署与运维指南
posts/              博客文章（当前有 1 篇示例 welcome.md）
data/               评论存储（含 README.md 格式说明）
static/uploads/     图床上传目录（当前为空）
scripts/            运维脚本
  make-font-subset.py   字体子集化
  push-oss.ps1          推送到 GitHub
deploy/
  wanle-backend.service  systemd 服务单元
.env.example        环境变量模板
LICENSE             MIT
```

## 功能

- **文章**：Markdown + YAML front matter，草稿/发布状态，分类与标签
- **编辑器**：全屏纯文本编辑整个 `.md`（含 front matter），保存前校验 YAML
- **评论**：嵌套回复、审核队列、B 站式编辑器、粘贴截图上传
- **图床**：图片无损重编码，支持粘贴/拖拽上传
- **账号**：JWT 登录、bcrypt 密码、邮箱验证码、登录失败限速
- **后台**：文章表格支持行内编辑、批量删除

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust 1.85+ · actix-web 4 · rusqlite · jsonwebtoken · bcrypt · lettre |
| 前端 | Vue 3 · vue-router 4 · axios · marked · highlight.js · Vite 5 |
| 部署 | Nginx 托管静态文件，反代 `/api/`、`/static/` |

## 存储设计

| 内容 | 位置 | 是否在仓库中 |
|---|---|---|
| 文章 | `posts/*.md` | ✅ 有示例 |
| 评论 | `data/comments/<slug>.json` | 目录存在，内容为空 |
| 上传记录 | `data/wanle.db`（SQLite） | ❌ gitignore，首次启动自动创建 |
| 用户 | `data/users.txt` | ❌ gitignore，首次启动自动创建 |
| 个人资料 | `data/profiles/<username>.json` | ❌ gitignore |
| 验证码 | `data/verifications.json` | ❌ gitignore |
| 上传文件 | `static/uploads/` | ❌ gitignore（保留 .gitkeep） |

为什么上传记录用 SQLite 而不是文件：每次插图都是一次写，且需按上传者筛选，索引比遍历目录快。

## 快速开始

### 1. 准备

需要 Rust 1.85+ 和 Node.js 18+。

```bash
cp .env.example .env
# 至少设置 JWT_SECRET；ADMIN_PASSWORD 留空则随机生成
```

### 2. 启动后端

```bash
cd backend-rust
cargo run
# 默认 http://127.0.0.1:8000
```

首次启动会自动创建管理员账号。设了 `ADMIN_PASSWORD` 就用它，否则随机生成并**打印在启动日志里**。

### 3. 启动前端

```bash
cd frontend
npm install
npm run dev
# http://127.0.0.1:5173，/api 与 /static 自动代理到 8000
```

### 4. 生产构建

```bash
cd frontend && npm run build          # 产物在 frontend/dist
cd ../backend-rust && cargo build --release
```

把 `frontend/dist` 同步到 Nginx 静态根目录，并反代 `/api/`、`/static/` 到后端。
参考 `nginx/nginx.conf`，systemd 服务参考 `deploy/wanle-backend.service`。

## 环境变量

`.env.example` 提供了基础配置；下面是代码中实际读取的全部变量。

### 基础

| 变量 | 说明 | 默认 |
|---|---|---|
| `BIND_ADDR` | 监听地址 | `127.0.0.1:8000` |
| `WANLE_ROOT` | 内容根目录 | `..` |
| `ADMIN_USERNAME` | 首次启动创建的管理员名 | `admin` |
| `ADMIN_PASSWORD` | 管理员密码，留空则随机生成并打印 | 随机 |

### 认证

| 变量 | 说明 | 默认 |
|---|---|---|
| `JWT_SECRET` | 签名密钥；不设则随机生成（重启后登录失效） | — |
| `JWT_SECRET_FILE` | 密钥文件路径（优先于 `JWT_SECRET`） | — |
| `JWT_TTL_HOURS` | 登录有效期（小时） | `24` |

### 存储路径（一般不需要改）

`POSTS_DIR`、`DATA_DIR`、`COMMENTS_DIR`、`USERS_FILE`、`STATIC_DIR`、`UPLOADS_DIR`

### 邮件验证码

| 变量 | 说明 |
|---|---|
| `MAIL_MODE` | 邮件模式 |
| `SMTP_HOST` / `SMTP_PORT` | SMTP 服务器 |
| `SMTP_USER` / `SMTP_PASS` | 账号与授权码（QQ 邮箱需用授权码，不是登录密码） |
| `SMTP_FROM` | 发件人显示 |
| `VERIFY_TTL_MINUTES` | 验证码有效期（分钟） |
| `VERIFY_RESEND_SECONDS` | 重发间隔（秒） |

### 视频转码

| 变量 | 说明 |
|---|---|
| `FFMPEG_BIN` | ffmpeg 可执行文件路径 |
| `FFMPEG_ENCODER` | 编码器名（部分发行版 ffmpeg 不含 libx264） |

### 其他

| 变量 | 说明 |
|---|---|
| `WANLE_COMMENT_REVIEW` | 评论是否需要审核 |

> **注意**：后端默认从 `backend-rust/` 目录启动，所以内容根目录是 `..`。
> **在项目根目录启动会导致内容目录指到外面，表现为「文章全没了」。**

## 部署

### 自建服务器（推荐，数据可持久保存）

```bash
cd frontend && npm run build          # 产物在 frontend/dist
cd ../backend-rust && cargo build --release
```

把 `frontend/dist` 同步到 Nginx 静态根目录，反代 `/api/`、`/static/` 到后端。
参考 `nginx/nginx.conf`，systemd 服务参考 `deploy/wanle-backend.service`。

### Docker

```bash
# 构建上下文必须是仓库根目录（Dockerfile 需要 posts/、data/、static/）
docker build -f backend-rust/Dockerfile -t wanle-backend .
docker run -p 8000:8000 -e BIND_ADDR=0.0.0.0:8000 wanle-backend
```

### Render（可选，免费实例）

仓库根目录的 `render.yaml` 可用于 [Render Blueprint](https://dashboard.render.com/blueprints) 一键创建免费实例。

**免费层的限制**（平台限制，非本项目问题）：

| 限制 | 影响 |
|---|---|
| 无持久磁盘 | 重启/重新部署后，写入的文章、评论、上传图片**全部重置** |
| 15 分钟无访问休眠 | 下次请求冷启动需数秒到一分钟 |

免费实例只适合临时演示界面，不适合真正写作。长期使用请部署到有持久存储的服务器。

### 环境变量补充

| 变量 | 说明 | 默认 |
|---|---|---|
| `PORT` | 云平台注入的端口，设置后自动绑定 `0.0.0.0:$PORT` | — |
| `CORS_ORIGINS` | 允许跨域的来源，逗号分隔；`*` 表示全部放行 | 空（不放行） |

`BIND_ADDR` 优先级高于 `PORT`。云平台必须监听 `0.0.0.0`，否则平台探测不到端口会报部署失败。

## 在线展示

本项目已部署上线：**<https://sakesoul.top/>** —— 可访问真实站点体验完整功能。

`docs/index.html` 是 GitHub Pages 上的静态展示页，用于介绍项目并提供跳转到线上站点的入口：

| 项 | 值 |
|---|---|
| 展示页地址 | <https://didididadida.github.io/rust-vue-website/> |
| 启用方式 | 仓库 Settings → Pages → Source 选 `main` 分支的 `/docs` 目录 |
| 内容 | 功能列表、架构图、存储设计、快速开始、跳转线上站点 |

> GitHub Pages 只托管静态文件，不能运行后端。所以展示页不内嵌活数据，
> 而是提供一个跳转到线上真实站点的入口。

## 文档

| 文档 | 内容 |
|---|---|
| [README.md](README.md) | 项目介绍、快速开始、接口一览（本文） |
| [docs/运维手册.md](docs/运维手册.md) | 部署、systemd、Nginx、备份恢复、故障排查 |
| [docs/Waline评论部署.md](docs/Waline评论部署.md) | 评论服务（waline-mini）部署与前端接入 |
| [docs/第三方设计来源.md](docs/第三方设计来源.md) | 前端视觉参考的上游项目与照抄范围（MIT 署名） |
| [data/README.md](data/README.md) | 内容文件格式（文章、评论、用户表） |
| [在线站点](https://sakesoul.top/) | 本项目实际部署的站点 |
| [展示页](https://didididadida.github.io/rust-vue-website/) | GitHub Pages 静态介绍 |

## 主要接口

统一响应体 `{ "code": 0, "message": "success", "data": ... }`，出错时 `code` 为 HTTP 状态码。

| 方法 | 路径 | 权限 | 说明 |
|---|---|---|---|
| GET | `/api/health` | 公开 | 健康检查 |
| POST | `/api/login` | 公开 | 登录，返回 JWT |
| POST | `/api/register` | 公开 | 注册 |
| POST | `/api/register/send-code` | 公开 | 发送邮箱验证码 |
| GET | `/api/me` | 登录 | 当前用户信息 |
| PUT | `/api/profile` | 登录 | 更新资料 |
| GET | `/api/articles` | 公开 | 文章列表（`scope=home/community/mine`）|
| GET/PUT | `/api/articles/{slug}/raw` | 管理员 | 原始 `.md` 读写 |
| POST | `/api/articles/batch-delete` | 管理员 | 批量删除 |
| POST | `/api/articles` | 管理员 | 新建文章 |
| PUT/DELETE | `/api/articles/{slug}` | 管理员 | 更新/删除 |
| GET/POST | `/api/articles/{slug}/comments` | 公开 | 评论读写 |
| GET | `/api/comments` | 管理员 | 全部评论 |
| PUT | `/api/comments/{slug}/{id}/approve` | 管理员 | 审核评论 |
| POST | `/api/upload` | 登录 | 上传图片/视频 |
| GET | `/api/images` | 登录 | 图片列表 |
| DELETE | `/api/images/{filename}` | 登录 | 删除图片 |

完整列表见 `backend-rust/src/handlers/mod.rs`。

## 脚本

| 脚本 | 用途 |
|---|---|
| `scripts/make-font-subset.py` | 字体子集化，裁到站内实际用到的字 |
| `scripts/push-oss.ps1` | 推送到 GitHub（token 走 `GITHUB_TOKEN` 环境变量）|

## 已知的坑

- **路径不能含非 ASCII 字符**。Windows + MinGW 链接 Rust 时读不到含中文/日文的路径，报「找不到 .o 文件」。
- **换行符**。Windows 的 `core.autocrlf=true` 会把 LF 转 CRLF，推到 Linux 后整个工作区显示「已修改」。建议 `git config core.autocrlf false`。
- **后端必须在 `backend-rust/` 目录启动**，否则内容路径错位。
- **PowerShell 脚本不要含中文注释**。Windows PowerShell 5.1 把无 BOM 的 UTF-8 文件按 ANSI 解码，中文会破坏引号配对导致语法错误。

## 许可

[MIT](LICENSE)
