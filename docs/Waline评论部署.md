# Waline 评论服务部署

> 本文说明如何在服务器上部署 [waline-mini](https://github.com/JQiue/waline-mini)（Rust 实现的 Waline），
> 以及前端如何接入。文章正文与评论区的关系见 [README.md](../README.md)。

## 一、为什么用 waline-mini 而不是自研评论

项目最初自己实现了一套评论（`comments` 表 + 手写审核），后来改为 Waline。原因：

| 能力 | 自研 | Waline |
|---|---|---|
| 嵌套回复 | ✅ 自己实现 | ✅ 自带 |
| 审核队列 | ✅ 自己实现 | ✅ 自带 |
| 反垃圾 | ❌ 无 | ✅ Akismet + 频率限制 + 违禁词 |
| 邮件通知 | ✅ 但需配置 SMTP | ✅ 自带，支持模板 |
| 表情 / Markdown / LaTeX | 仅 Markdown | ✅ 全套 |
| 数据备份 | ❌ 无 | ✅ 后台可导出 |
| 防丢失草稿 | ❌ 无 | ✅ 自动保存 |
| 多账号登录 | ❌ 仅本站账号 | ✅ GitHub / Google / 邮箱 |

**选 waline-mini 而非原版 Waline 的理由**：原版是 Node.js 实现，内存占用约 130MB；
waline-mini 是 Rust 实现，实测 **15MB**，适合本项目 2GB 内存的服务器。

## 二、部署步骤

### 1. 下载二进制

```bash
# 从 GitHub Releases 下载对应平台
curl -L -o waline-mini.tar.gz \
  https://github.com/JQiue/waline-mini/releases/download/v0.13.0/waline-mini-v0.13.0-x86_64-unknown-linux-gnu.tar.gz

mkdir -p /opt/waline/data
tar xzf waline-mini.tar.gz
mv waline-mini /opt/waline/
chmod +x /opt/waline/waline-mini
```

### 2. 配置环境变量

```bash
cat > /opt/waline/.env << 'EOF'
# 数据库（SQLite，无需额外服务）
DATABASE_URL=sqlite:///opt/waline/data/waline.sqlite?mode=rwc

# JWT 密钥，用 openssl rand -hex 32 生成
JWT_TOKEN=你的随机字符串

SITE_NAME=站点名称
SITE_URL=https://your-domain.com
SERVER_URL=https://your-domain.com

HOST=127.0.0.1
PORT=8360
WORKERS=1

# 审核开关：true 时所有评论需管理员批准
COMMENT_AUDIT=false

# 反垃圾：false 表示关闭 Akismet
AKISMET_KEY=false
EOF

chmod 600 /opt/waline/.env
```

**关键配置说明**：

| 变量 | 说明 |
|---|---|
| `DATABASE_URL` | 必须带 `?mode=rwc`，否则数据库文件不存在时启动失败 |
| `HOST` | 设 `127.0.0.1`，只监听本地，由 Nginx 转发 |
| `PORT` | 默认 8360，与主后端 8000 不冲突 |
| `WORKERS` | 1 即可（个人站并发低，多 worker 反而增加内存） |

### 3. systemd 服务

```bash
cat > /etc/systemd/system/waline-mini.service << 'EOF'
[Unit]
Description=Waline Mini comment system
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
Group=root
WorkingDirectory=/opt/waline
EnvironmentFile=/opt/waline/.env
ExecStart=/opt/waline/waline-mini

Restart=always
RestartSec=3
StartLimitIntervalSec=10
StartLimitBurst=5

StandardOutput=journal
StandardError=journal
SyslogIdentifier=waline-mini

NoNewPrivileges=yes
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now waline-mini
systemctl status waline-mini
```

首次启动会自动建表（日志里能看到 `Applying migration 'migration_01_init_table'`）。

### 4. Nginx 反向代理

Waline 客户端的默认请求路径是 `/api/*`，与主后端冲突，因此挂在 `/comments/` 下：

```nginx
location /comments/ {
    proxy_pass http://127.0.0.1:8360/;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;

    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
}
```

注意 `proxy_pass` 末尾的 `/`：它会把 `/comments/` 前缀剥掉，转成 Waline 期望的 `/api/...`。

```bash
nginx -t && systemctl reload nginx
```

### 5. 验证

```bash
# 列表接口（必须带 sortBy，见下方"已知问题"）
curl -s "http://127.0.0.1:8360/api/comment?path=/blog/test&lang=zh-CN&pageSize=10&page=1&sortBy=insertedAt_desc"
```

正常返回：

```json
{"errno":0,"errmsg":"","data":{"count":0,"data":[]}}
```

## 三、前端接入

前端用 `@waline/client`，组件在 `frontend/src/components/WalineComments.vue`：

```js
const { init } = await import('@waline/client')
await import('@waline/client/style')

instance = init({
  el: host.value,
  serverURL: '/comments',        // Nginx 反代地址
  path: `/blog/${props.slug}`,   // 评论归属路径
  lang: 'zh-CN',
  login: 'disable',              // 只允许匿名昵称评论
})
```

**两个必须注意的点**：

1. **`path` 必须与后端数据的 `url` 字段一致**。前端传 `/blog/<slug>`，
   迁移脚本也必须写 `/blog/<slug>`，否则历史评论显示不出来。
2. **组件卸载要调用 `destroy()`**，否则切换文章会残留旧实例。

## 四、数据迁移

如果已有自研评论数据（`comments` 表），迁移到 Waline：

```bash
python3 migrate_comments.py /path/to/wanle.db /opt/waline/data/waline.sqlite
```

脚本特性：

- 按 `(url, nick, comment)` 判重 → **幂等**，可重复执行
- 保留原始 `created_at`
- 未审核评论映射为 `status='waiting'`

## 五、已知问题

### `sortBy` 参数是必需的

waline-mini 的列表接口**不带 `sortBy` 会返回空列表**（`errno: 1000`，`data` 缺失），
不报错、不提示。这是与官方 Waline 的行为差异。

前端 `WalineComments.vue` 里查询评论数时已显式带上 `sortBy=insertedAt_desc`。

### 评论计数接口

`type=count` 需要额外的 `page` 参数，缺失时返回 400。前端已做容错——
计数失败不影响评论列表显示。

### 不提供 IP 归属地

日志里会有 `xdb is not provided` 提示。要显示评论者地区需要额外提供 IP 库文件
（`IP2REGION_DB`）。个人站可忽略。

## 六、备份与恢复

Waline 的数据全部在 `waline.sqlite` 一个文件里：

```bash
# 备份
cp /opt/waline/data/waline.sqlite ~/waline-backup-$(date +%Y%m%d).sqlite

# 恢复
systemctl stop waline-mini
cp ~/waline-backup-20260101.sqlite /opt/waline/data/waline.sqlite
systemctl start waline-mini
```

## 七、运维命令

```bash
# 状态
systemctl status waline-mini

# 日志
journalctl -u waline-mini -f

# 重启
systemctl restart waline-mini

# 内存占用
ps -o rss= -p $(systemctl show -p MainPID --value waline-mini) | awk '{printf "%.1f MB\n", $1/1024}'
```

## 八、依据

**经典问题**：第三方组件的生命周期与数据归属

- **来源**：
  - Waline 官方客户端文档 <https://waline.js.org/reference/client/>
  - Valaxy 官方评论插件 <https://valaxy.site/zh/addons/official/waline>
  - waline-mini 仓库 <https://github.com/JQiue/waline-mini>
- **原型结论**：
  1. `init()` 返回带 `destroy()` 的实例，切换页面必须销毁重建；
  2. 评论以 `path`（页面标识）归属，服务端按此分组。
- **本处变体**：
  - Valaxy 用插件自动注册组件，本站是纯 Vue SPA，直接调 `init()`；
  - 需要适配站点暗色主题，通过覆盖 Waline 的 CSS 变量实现。
- **沿用结论**：`serverURL` + `path` 两个核心参数；`destroy()` 配对调用。
- **偏离处**：未开启 pageview / reaction（本站自己统计浏览数），减少额外请求。
