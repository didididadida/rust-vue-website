# =============================================================================
# 后端镜像（构建上下文 = 仓库根目录）
#
# 构建：
#   docker build -f backend-rust/Dockerfile -t wanle-backend .
# 运行（本地）：
#   docker run -p 8000:8000 -e BIND_ADDR=0.0.0.0:8000 wanle-backend
# 运行（Render/Railway）：平台注入 PORT，程序自动绑 0.0.0.0:$PORT
#
# 依据：
#   经典问题：多阶段构建（multi-stage build）与构建上下文边界
#   来源：Docker 官方文档 "Multi-stage builds" 与 "Build context"
#        https://docs.docker.com/build/building/multi-stage/
#        https://docs.docker.com/build/building/context/
#   原型结论：
#     1. COPY 只能引用构建上下文内的路径，`../xxx` 一律失败；
#     2. 多阶段构建让编译产物与工具链分离，运行镜像可小到几十 MB。
#   本处变体：Dockerfile 放在 backend-rust/，但需要同时带入仓库根的
#             posts/、data/、static/，所以上下文必须设为仓库根。
#   沿用结论：上下文根 = 仓库根，Dockerfile 用 -f 指定，路径写 backend-rust/...
#   偏离处：构建上下文包含整个仓库，首次传输略大；用 .dockerignore 收敛。
# =============================================================================

# ---------- 构建阶段 ----------
FROM rust:1-slim-bookworm AS builder

# ring / rusqlite(bundled) 需要 C 工具链
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 先复制清单，利用分层缓存：依赖没变时不重新编译依赖
COPY backend-rust/Cargo.toml backend-rust/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src target/release/deps/wanle_backend* target/release/wanle-backend*

# 再复制真实源码
COPY backend-rust/src ./src
RUN cargo build --release

# ---------- 运行阶段 ----------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/wanle-backend /app/wanle-backend

# 内容目录：预置一份，进程启动时 ensure_dirs() 会补齐缺失的子目录
# 注意：云平台免费层是临时文件系统，重启后这里的改动会丢失
COPY posts /app/posts
COPY data /app/data
COPY static /app/static

# 内容根目录 = /app（posts、data、static 都在这一层）
ENV WANLE_ROOT=/app
ENV RUST_LOG=info

EXPOSE 8000

CMD ["/app/wanle-backend"]
