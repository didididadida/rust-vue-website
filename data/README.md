# 数据目录（纯文件，无数据库）

```
data/
├── users.txt      # 用户表，一行一个账号
├── toys/          # 自制项目，一个项目一个 .md
├── tools/         # 实用工具，一个工具一个 .md
└── comments/      # 评论，一篇文章一个 .json
```

## 文章：`posts/*.md`

```markdown
---
title: 欢迎来到 wanle
slug: hello-world
summary: 第一篇博客文章
category: blog          # blog / essay / tech
tags: [博客, 随笔]       # 也可以写成：tags: 博客, 随笔
cover_image: ""
is_published: true      # false 时为草稿，仅管理员可见
created_at: 2026-08-25T01:33:00Z
updated_at: 2026-08-25T01:33:00Z
---

正文用 Markdown 编写。
```

## 项目：`data/toys/*.md`

```markdown
---
name: 项目名称
description: 一句话介绍（卡片上显示）
image: /static/uploads/xxx.png
demo_url: https://...
source_url: https://github.com/...
tech_stack: [Vue, Rust]     # 也可以写成：Vue, Rust
is_published: true
created_at: 2026-08-25T01:33:00Z
updated_at: 2026-08-25T01:33:00Z
---

正文用 Markdown 编写，展示在 `/toys/<slug>` 详情页（支持标题、列表、代码块、图片、表格）。
```

## 工具：`data/tools/*.md`

```markdown
---
name: 工具名称
description: 一句话介绍（卡片上显示）
icon: 🔧
category: utility       # dev / utility / productivity / other
url: https://...
is_published: true
created_at: 2026-08-25T01:33:00Z
updated_at: 2026-08-25T01:33:00Z
---

正文用 Markdown 编写，展示在 `/tools/<slug>` 详情页。
```

## 用户：`data/users.txt`

```text
# 每行格式：用户名:bcrypt 密码哈希:角色(admin|user)
admin:$2b$12$....:admin
reader:$2b$12$....:user
```

## 评论：`data/comments/<slug>.json`

一篇文章一个文件，文件名就是文章 slug，内容是一个 JSON 数组：

```json
[
  {
    "id": "c-9ece2bcb599b",
    "parent_id": null,
    "author": "user",
    "email": "",
    "content": "写得不错！",
    "is_approved": true,
    "created_at": "2026-10-14T08:03:22Z"
  }
]
```

- `parent_id` 指向被回复的评论 `id`；顶层评论省略该字段。
- `is_approved: false` 表示待审核，公开接口不会返回。
- `email` 只在后台可见（`GET /api/comments` 需要管理员登录）。
- 直接编辑该文件即可改评论；删掉整篇文章时对应文件会被自动清理。

- 文件名（去掉 `.md`）就是该条内容的 `slug`，也是 URL 中使用的标识，请勿包含 `/ \ : * ? " < > |`。
- 管理员可增删改所有内容；`user` 角色只能登录并使用图床。
- 直接手写、编辑这些文件即可，无需重启服务，下次请求即生效。
- `data/users.txt` 已在 `.gitignore` 中，不会被提交。
