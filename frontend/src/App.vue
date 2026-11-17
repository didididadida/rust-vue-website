<template>
  <div class="app">
    <!-- 阅读进度条 -->
    <div class="progress-bar" :style="{ transform: `scaleX(${progress})` }" aria-hidden="true"></div>

    <header class="header" :class="{ 'is-scrolled': scrolled }">
      <div class="header-inner">
        <router-link to="/" class="logo">
          <span class="logo-mark">飼</span>
          <span class="logo-text">飼酒の小站</span>
        </router-link>

        <!-- 移动端菜单按钮 -->
        <button
          class="menu-toggle"
          type="button"
          :aria-expanded="menuOpen"
          aria-label="切换导航菜单"
          @click="menuOpen = !menuOpen"
        >
          <span :class="{ open: menuOpen }"></span>
        </button>

        <nav class="nav" :class="{ 'is-open': menuOpen }" @click="menuOpen = false">
          <router-link to="/">首页</router-link>
          <router-link to="/blog">文章</router-link>
          <!-- 社区只对登录用户显示：未登录的人看不到入口 -->

          <template v-if="isLoggedIn">
            <!-- 图床仅管理员可见：普通用户的图片上传已内建在编辑器里，不需要单独的图床页 -->
            <router-link v-if="auth.isAdmin" to="/images">图床</router-link>
            <router-link v-if="auth.isAdmin" to="/admin">管理</router-link>

            <!-- 当前用户：头像 + 昵称，点进个人主页 -->
            <router-link to="/me" class="nav-user" title="我的主页">
              <img v-if="auth.avatar" :src="auth.avatar" :alt="displayName" class="nav-avatar" />
              <span v-else class="nav-avatar nav-avatar-fallback">{{ initial(displayName) }}</span>
              <span class="nav-user-name">{{ displayName }}</span>
            </router-link>

            <button class="logout-btn" type="button" @click="logout">退出</button>
          </template>

          <template v-else>
            <router-link to="/register" class="nav-join">注册</router-link>
            <router-link to="/login" class="nav-login">登录</router-link>
          </template>
        </nav>
      </div>
    </header>

    <main class="main">
      <router-view v-slot="{ Component }">
        <transition name="fade" mode="out-in">
          <component :is="Component" />
        </transition>
      </router-view>
    </main>

    <footer class="footer">
      <div class="footer-inner">
        <p class="footer-line">© {{ year }} 飼酒の小站 · All at sea. <span class="heart">♥</span></p>
      </div>
    </footer>
  </div>
</template>

<script setup>
import { onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import http from './api'
import { auth, isLoggedIn, displayName, setAuth, clearAuth } from './auth'

const router = useRouter()
const year = new Date().getFullYear()
const scrolled = ref(false)
const menuOpen = ref(false)
const progress = ref(0)

/** 头像兜底：取昵称首字符 */
const initial = name => (name || '?').trim().charAt(0).toUpperCase()

/**
 * 滚动处理：同时更新顶栏状态与阅读进度。
 * 用 requestAnimationFrame 节流 —— scroll 事件触发频率远高于渲染帧，
 * 直接在每个事件里改响应式数据会引发大量冗余重渲染，长文章下明显掉帧。
 */
let ticking = false
const onScroll = () => {
  if (ticking) return
  ticking = true
  requestAnimationFrame(() => {
    scrolled.value = window.scrollY > 8

    const doc = document.documentElement
    // 可滚动高度为 0 时（内容不足一屏）视为已读完
    const max = doc.scrollHeight - window.innerHeight
    progress.value = max > 0 ? Math.min(1, Math.max(0, window.scrollY / max)) : 1

    ticking = false
  })
}

// 路由切换后页面高度会变，需重新计算进度
router.afterEach(() => {
  requestAnimationFrame(onScroll)
})

onMounted(async () => {
  window.addEventListener('scroll', onScroll, { passive: true })
  window.addEventListener('resize', onScroll, { passive: true })
  onScroll()

  // 启动时校验本地 token 是否仍然有效，并同步最新资料
  if (!auth.token) return
  try {
    const me = await http.get('/api/me')
    setAuth({
      token: auth.token,
      username: me.username,
      nickname: me.nickname,
      avatar: me.avatar,
      is_admin: me.is_admin
    })
  } catch {
    clearAuth()
  }
})

onUnmounted(() => {
  window.removeEventListener('scroll', onScroll)
  window.removeEventListener('resize', onScroll)
})

function logout() {
  clearAuth()
  menuOpen.value = false
  router.push('/')
}
</script>

<style scoped>
.app {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

/* ---------- 阅读进度条：细冷光线 ---------- */
/* 用 transform: scaleX 而非改 width —— 只走合成层，不触发布局重排 */
.progress-bar {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: 1.5px;
  z-index: 200;
  transform-origin: 0 50%;
  background: linear-gradient(90deg, var(--nebula), var(--accent), #b6f0ff);
  box-shadow: 0 0 10px rgba(76, 201, 240, 0.55);
  transition: transform 0.1s linear;
}

/* ---------- 顶栏：深空玻璃 ---------- */
.header {
  position: sticky;
  top: 0;
  z-index: 100;
  background: rgba(5, 7, 15, 0.68);
  /* 顶栏是一直可见的 sticky 元素，模糊开销最大：
     降到 12px 且不用 saturate（saturate 每次都要跑色彩矩阵，代价高） */
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-bottom: 1px solid transparent;
  transition: background 0.3s var(--ease), border-color 0.3s var(--ease),
    box-shadow 0.3s var(--ease);
}

.header.is-scrolled {
  background: rgba(5, 7, 15, 0.93);
  border-bottom-color: var(--border);
  box-shadow: 0 1px 0 rgba(76, 201, 240, 0.08), 0 8px 30px rgba(0, 0, 0, 0.5);
}

.header-inner {
  max-width: var(--content-width);
  margin: 0 auto;
  padding: 0 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 68px;
  gap: 1rem;
}

.logo {
  display: inline-flex;
  align-items: center;
  gap: 0.7rem;
  text-decoration: none;
  flex-shrink: 0;
}

/* logo 标记：冷光方块，像一颗被框住的恒星 */
.logo-mark {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border-radius: 8px;
  background: radial-gradient(circle at 32% 28%, #b6f0ff, #2b7fd4 55%, #4a3aa8);
  color: #03050b;
  font-family: var(--font-serif);
  font-size: 1.05rem;
  font-weight: 600;
  box-shadow: 0 0 18px rgba(76, 201, 240, 0.45),
    inset 0 0 10px rgba(255, 255, 255, 0.35);
  transition: transform 0.35s var(--ease-out), box-shadow 0.35s var(--ease);
}

.logo:hover .logo-mark {
  transform: rotate(90deg) scale(1.06);
  box-shadow: 0 0 28px rgba(76, 201, 240, 0.7),
    inset 0 0 12px rgba(255, 255, 255, 0.5);
}

.logo-text {
  font-family: var(--font-serif);
  font-size: 1.14rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: #eaf2ff;
  transition: color 0.25s var(--ease);
}

.logo:hover .logo-text {
  color: var(--accent-soft);
}

/* ---------- 导航 ---------- */
.nav {
  display: flex;
  align-items: center;
  gap: 0.1rem;
}

.nav a {
  position: relative;
  color: var(--text-secondary);
  text-decoration: none;
  font-size: 0.9rem;
  letter-spacing: 0.03em;
  padding: 0.4rem 0.78rem;
  border-radius: var(--radius-sm);
  transition: color 0.22s var(--ease), background 0.22s var(--ease);
}

.nav a:hover {
  color: var(--text-primary);
  background: rgba(126, 156, 210, 0.08);
}

.nav a.router-link-active {
  color: var(--accent);
}

/* 当前页：底部一条冷光短线 */
.nav a.router-link-active::after {
  content: '';
  position: absolute;
  left: 0.78rem;
  right: 0.78rem;
  bottom: 0.02rem;
  height: 1.5px;
  border-radius: var(--radius-full);
  background: var(--accent);
  box-shadow: 0 0 8px rgba(76, 201, 240, 0.8);
}

.nav-login {
  border: 1px solid rgba(76, 201, 240, 0.3) !important;
  color: var(--accent-soft) !important;
  margin-left: 0.5rem;
}

.nav-login:hover {
  background: rgba(76, 201, 240, 0.12) !important;
  border-color: var(--accent) !important;
}

.nav-join {
  color: var(--text-muted) !important;
  margin-left: 0.3rem;
}

/* ---------- 顶栏当前用户 ---------- */
.nav-user {
  display: inline-flex !important;
  align-items: center;
  gap: 0.45rem;
  margin-left: 0.4rem;
  padding: 0.22rem 0.7rem 0.22rem 0.24rem !important;
  border: 1px solid var(--border);
  border-radius: var(--radius-full) !important;
}

.nav-user:hover {
  border-color: rgba(76, 201, 240, 0.32);
  background: rgba(76, 201, 240, 0.07) !important;
}

/* 用户在顶栏时不要显示底部指示条，圆角胶囊已足够标示 */
.nav-user.router-link-active::after {
  display: none;
}

.nav-avatar {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  object-fit: cover;
  flex-shrink: 0;
  border: 1px solid rgba(76, 201, 240, 0.3);
}

.nav-avatar-fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-serif);
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--accent-soft);
  background: radial-gradient(circle at 35% 30%, #123049, #0a0f1d 70%);
}

.nav-user-name {
  font-size: 0.86rem;
  max-width: 8em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.logout-btn {
  background: none;
  border: 1px solid var(--border-strong);
  color: var(--text-muted);
  border-radius: var(--radius-sm);
  padding: 0.32rem 0.85rem;
  font-size: 0.83rem;
  font-family: inherit;
  letter-spacing: 0.03em;
  cursor: pointer;
  margin-left: 0.5rem;
  transition: all 0.22s var(--ease);
}

.logout-btn:hover {
  border-color: rgba(248, 113, 113, 0.5);
  color: var(--danger);
}

/* ---------- 移动端菜单按钮 ---------- */
.menu-toggle {
  display: none;
  width: 40px;
  height: 40px;
  border: 1px solid var(--border-strong);
  background: transparent;
  border-radius: var(--radius-sm);
  cursor: pointer;
  position: relative;
}

.menu-toggle span,
.menu-toggle span::before,
.menu-toggle span::after {
  position: absolute;
  left: 50%;
  width: 15px;
  height: 1.5px;
  background: var(--text-secondary);
  border-radius: 2px;
  transform: translateX(-50%);
  transition: transform 0.28s var(--ease), opacity 0.2s var(--ease),
    background 0.2s var(--ease);
}

.menu-toggle span {
  top: 50%;
  margin-top: -0.75px;
}

.menu-toggle span::before {
  content: '';
  top: -5px;
}

.menu-toggle span::after {
  content: '';
  top: 5px;
}

.menu-toggle span.open {
  background: transparent;
}

.menu-toggle span.open::before {
  transform: translateX(-50%) rotate(45deg);
  top: 0;
  background: var(--accent);
}

.menu-toggle span.open::after {
  transform: translateX(-50%) rotate(-45deg);
  top: 0;
  background: var(--accent);
}

/* ---------- 主内容 ---------- */
.main {
  flex: 1;
  max-width: var(--content-width);
  width: 100%;
  margin: 0 auto;
  padding: 2.75rem 1.5rem 4.5rem;
  position: relative;
}

/* ---------- 页脚 ---------- */
.footer {
  position: relative;
  border-top: 1px solid var(--border);
  padding: 2rem 1.5rem;
}

.footer::before {
  content: '';
  position: absolute;
  top: -1px;
  left: 50%;
  transform: translateX(-50%);
  width: 40%;
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--accent), transparent);
  opacity: 0.4;
}

.footer-inner {
  max-width: var(--content-width);
  margin: 0 auto;
  text-align: center;
}

.footer-line {
  color: var(--text-muted);
  font-size: 0.82rem;
  font-family: var(--font-mono);
  letter-spacing: 0.06em;
}

/* 页脚那颗星：缓慢呼吸 */
.footer-line .heart {
  color: var(--accent);
  display: inline-block;
  margin-left: 0.25rem;
  animation: starPulse 3.4s ease-in-out infinite;
}

/* ---------- 路由过渡 ---------- */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.22s var(--ease), transform 0.22s var(--ease-out);
}

.fade-enter-from {
  opacity: 0;
  transform: translateY(8px);
}

.fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

/* ---------- 响应式 ---------- */
@media (max-width: 860px) {
  .header-inner {
    height: 60px;
    padding: 0 1rem;
  }

  .menu-toggle {
    display: block;
  }

  .nav {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    flex-direction: column;
    align-items: stretch;
    gap: 0.15rem;
    padding: 0.6rem 1rem 1rem;
    background: rgba(5, 7, 15, 0.97);
    backdrop-filter: blur(10px);
    border-bottom: 1px solid var(--border);
    opacity: 0;
    visibility: hidden;
    transform: translateY(-8px);
    transition: opacity 0.25s var(--ease), transform 0.25s var(--ease-out),
      visibility 0.25s;
  }

  .nav.is-open {
    opacity: 1;
    visibility: visible;
    transform: translateY(0);
  }

  .nav a,
  .logout-btn {
    padding: 0.7rem 0.9rem;
    margin-left: 0;
    text-align: left;
    width: 100%;
  }

  .nav a.router-link-active::after {
    display: none;
  }

  .nav a.router-link-active {
    background: rgba(76, 201, 240, 0.08);
  }

  .nav-login {
    border-color: transparent !important;
    margin-top: 0.25rem;
  }

  .nav-join {
    margin-left: 0;
  }

  .nav-user {
    margin-left: 0;
    border-radius: var(--radius-sm) !important;
    padding: 0.6rem 0.9rem !important;
  }

  .nav-user-name {
    max-width: none;
  }

  .nav-avatar {
    width: 22px;
    height: 22px;
  }

  .logo-text {
    font-size: 1.02rem;
    letter-spacing: 0.05em;
  }

  .logo-mark {
    width: 30px;
    height: 30px;
    font-size: 0.95rem;
  }

  .main {
    padding: 1.75rem 1rem 3rem;
  }
}
</style>
