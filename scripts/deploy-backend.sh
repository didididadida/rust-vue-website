#!/bin/bash
# =============================================================================
# 部署新版本后端（SQLite + SeaORM）到线上
#
# 逻辑描述：
#   输入 → 输出：新二进制 → 替换线上 wanle-backend，保留完整回滚点
#   关键机制：
#     1. 部署前备份：数据库目录、posts/、当前二进制、systemd 单元
#     2. 停服务 → 换二进制 → 跑迁移 → 起服务 → 验证
#     3. 任一步失败立即回滚并打印原因
#   不变量：
#     - 旧二进制始终保留在备份目录里，随时可回滚
#     - 迁移只新增数据，不删除源文件
#   边界：
#     - 迁移幂等，重跑不会重复插入
#     - 回滚只恢复二进制，数据库是新增的，不影响旧版本（旧版本不读它）
#
# 依据：
#   经典问题：可回滚的部署（rollback-safe deployment）
#   来源：
#     1. 本机全局约定「脚本与代码规范」第 5 节：
#        "每次部署都要给出回滚点（备份目录 + 恢复命令），没有回滚点不算部署完成"
#     2. 全局约定第 2 节：有副作用的操作必须先备份
#   原型结论：
#     1. 部署失败的代价取决于能否快速回到已知良好状态；
#     2. 备份必须包含"运行所需的一切"（二进制 + 数据 + 配置），
#        只备份代码是不够的。
#   本处变体：
#     - 这是单机部署，没有蓝绿/滚动更新的条件，只能"停-换-起"；
#     - 新旧版本共存的关键：旧版本用文件存储，新版本用 SQLite，
#       两者读的是不同位置，因此回滚不会造成数据格式冲突。
#   沿用结论：停服务 → 备份 → 替换 → 迁移 → 启动 → 验证，每步检查退出码。
#   偏离处：
#     - 没有做数据库 schema 的降级脚本（回滚只需恢复旧二进制，
#       旧版本不读 wanle.db，因此不需要降级）；
#     - 没有做灰度发布（单实例无此条件）。
# =============================================================================

set -euo pipefail

PROJECT_DIR="${PROJECT_DIR:-/root/wanle}"
NEW_BIN="${NEW_BIN:-/tmp/backend-full/target/release/wanle-backend}"
SERVICE="wanle-backend"
STAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_DIR="/root/wanle-deploy-backup-${STAMP}"
LIVE_BIN="${PROJECT_DIR}/backend-rust/target/release/wanle-backend"

log() { echo -e "\n\033[36m=== $* ===\033[0m"; }
ok()  { echo -e "  \033[32m[OK]\033[0m $*"; }
err() { echo -e "  \033[31m[FAIL]\033[0m $*"; }

rollback() {
    err "部署失败，开始回滚…"
    systemctl stop "$SERVICE" 2>/dev/null || true

    if [ -f "${BACKUP_DIR}/wanle-backend" ]; then
        cp "${BACKUP_DIR}/wanle-backend" "$LIVE_BIN"
        chmod +x "$LIVE_BIN"
        ok "已恢复旧二进制"
    fi

    systemctl start "$SERVICE" 2>/dev/null || true
    sleep 2

    if systemctl is-active --quiet "$SERVICE"; then
        ok "服务已用旧版本恢复运行"
    else
        err "服务仍无法启动，请手动检查：journalctl -u ${SERVICE} -n 50"
    fi

    echo ""
    echo "回滚点: ${BACKUP_DIR}"
    echo "手动恢复: cp ${BACKUP_DIR}/wanle-backend ${LIVE_BIN} && systemctl restart ${SERVICE}"
    exit 1
}

trap rollback ERR

# ---------- 0. 前置检查 ----------
log "0. 前置检查"

if [ ! -f "$NEW_BIN" ]; then
    err "找不到新二进制: $NEW_BIN"
    exit 1
fi
ok "新二进制: $(ls -lh "$NEW_BIN" | awk '{print $5}')"

if [ ! -d "$PROJECT_DIR" ]; then
    err "项目目录不存在: $PROJECT_DIR"
    exit 1
fi
ok "项目目录: $PROJECT_DIR"

# ---------- 1. 备份 ----------
log "1. 备份到 ${BACKUP_DIR}"
mkdir -p "$BACKUP_DIR"

# 当前二进制
if [ -f "$LIVE_BIN" ]; then
    cp "$LIVE_BIN" "${BACKUP_DIR}/wanle-backend"
    ok "已备份二进制"
fi

# 数据目录（数据库 + 评论 + 用户）
if [ -d "${PROJECT_DIR}/data" ]; then
    cp -r "${PROJECT_DIR}/data" "${BACKUP_DIR}/data"
    ok "已备份 data/ ($(du -sh "${PROJECT_DIR}/data" | cut -f1))"
fi

# 文章
if [ -d "${PROJECT_DIR}/posts" ]; then
    cp -r "${PROJECT_DIR}/posts" "${BACKUP_DIR}/posts"
    ok "已备份 posts/ ($(ls -1 "${PROJECT_DIR}/posts"/*.md 2>/dev/null | wc -l) 篇)"
fi

# 环境配置
if [ -f "${PROJECT_DIR}/.env" ]; then
    cp "${PROJECT_DIR}/.env" "${BACKUP_DIR}/.env"
    ok "已备份 .env"
fi

# systemd 单元
if [ -f "/etc/systemd/system/${SERVICE}.service" ]; then
    cp "/etc/systemd/system/${SERVICE}.service" "${BACKUP_DIR}/"
    ok "已备份 systemd 单元"
fi

# ---------- 2. 停服务 ----------
log "2. 停止服务"
systemctl stop "$SERVICE"
ok "服务已停止"

# ---------- 3. 替换二进制 ----------
log "3. 替换二进制"
mkdir -p "$(dirname "$LIVE_BIN")"
cp "$NEW_BIN" "$LIVE_BIN"
chmod +x "$LIVE_BIN"
ok "已替换: $(ls -lh "$LIVE_BIN" | awk '{print $5}')"

# ---------- 4. 跑迁移（一次性） ----------
log "4. 执行数据迁移"
cd "$PROJECT_DIR"

# AUTO_MIGRATE=1 让服务启动时自动迁移；这里先跑一次并看日志
export AUTO_MIGRATE=1
systemctl start "$SERVICE"
sleep 5

if ! systemctl is-active --quiet "$SERVICE"; then
    err "服务启动失败"
    journalctl -u "$SERVICE" -n 30 --no-pager
    exit 1
fi

# 打印迁移日志
journalctl -u "$SERVICE" -n 40 --no-pager | grep -E "迁移|当前数据|管理员|启动" || true

# ---------- 5. 验证 ----------
log "5. 验证"

echo "  等待服务就绪…"
for i in $(seq 1 10); do
    if curl -sf http://127.0.0.1:8000/api/health > /tmp/health.json 2>/dev/null; then
        break
    fi
    sleep 1
done

if [ -f /tmp/health.json ]; then
    ok "健康检查: $(cat /tmp/health.json)"
else
    err "健康检查失败"
    exit 1
fi

# 检查文章接口
echo ""
echo "  文章列表:"
curl -sf "http://127.0.0.1:8000/api/articles?limit=3" | head -c 400 || err "文章接口失败"
echo ""

# ---------- 完成 ----------
log "部署完成"
echo ""
echo "  备份（回滚点）: ${BACKUP_DIR}"
echo "  回滚命令:"
echo "    cp ${BACKUP_DIR}/wanle-backend ${LIVE_BIN}"
echo "    systemctl restart ${SERVICE}"
echo ""
echo "  查看日志: journalctl -u ${SERVICE} -f"
