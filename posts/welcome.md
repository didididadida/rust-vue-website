---
title: 欢迎使用 wanle
summary: 这是一个示例文章，展示 Markdown 与 front matter 的写法。可以直接删除或修改。
category: blog
tags:
- 示例
- 入门
cover_image: ''
is_published: true
review_status: approved
is_featured: false
created_at: 2026-01-01T00:00:00Z
updated_at: 2026-01-01T00:00:00Z
---

# 欢迎

这是 wanle 的示例文章。整篇文章就是一个 `.md` 文件，开头用 `---` 包起来的是元数据（front matter），下面是正文。

## 元数据字段

| 字段 | 说明 |
|---|---|
| `title` | 标题，必填 |
| `summary` | 摘要，列表页会显示 |
| `category` | 分类：`blog` / `essay` / `tech` |
| `tags` | 标签，数组形式 |
| `cover_image` | 封面图路径，可留空 |
| `is_published` | `true` 发布 / `false` 草稿（仅登录后可见） |

## Markdown 语法

支持标准的 Markdown。标题、**粗体**、*斜体*、`行内代码`、[链接](https://example.com)。

### 列表

- 无序列表项一
- 无序列表项二

1. 有序列表项一
2. 有序列表项二

### 引用

> 这是一段引用。
> 可以跨行。

### 代码块

代码块会自动语法高亮，右上角带一键复制按钮：

```rust
fn main() {
    println!("Hello, wanle!");
}
```

```javascript
const greet = (name) => `Hello, ${name}!`
console.log(greet('wanle'))
```

### 图片

在编辑器里可以直接**粘贴截图**，会自动上传并插入图片语法：

```markdown
![图片](/static/uploads/xxx.png)
```

## 开始写作

1. 登录后台 `/admin`
2. 点「新建文章」
3. 进入全屏编辑器，直接编辑整个 `.md` 文件
4. `Ctrl+S` 保存

整篇文章都是纯文本，你可以直接改 front matter 里的任何一个字段。

祝你写作愉快。
