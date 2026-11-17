<!--
  首页（参考 Valaxy 的视觉语言）

  逻辑描述：
    输入：最新文章列表（GET /api/articles?limit=N）
    输出：
      - 首屏大标题区（站点名 + 标语 + 联系方式），带打字机入场效果
      - 最新文章卡片列表
    关键机制：
      1. 标题用 CSS 逐字入场（animation-delay 按索引递增），
         不用 JS 定时器，避免卸载时残留 timer；
      2. 卡片布局参考 Valaxy：左侧封面（可选）、右侧标题+摘要+元信息；
      3. 尊重 prefers-reduced-motion：用户关闭动画时不播放。
    不变量：
      - 文章区在数据未就绪时显示骨架，不显示「空」；
      - 接口失败不影响首屏标题区显示。
    边界：
      - 无文章时显示引导文案而非报错；
      - 封面图缺失时卡片自动退化为纯文字布局。

  依据：
    经典问题：渐进式呈现与动效可访问性（progressive reveal & reduced motion）
    来源：
      1. Valaxy 官网首页的结构与视觉
         https://valaxy.site/ —— 大标题 + 标语 + 特性分块 + 卡片
      2. Valaxy 主题 Yun 的「首页标语」说明
         https://valaxy.site/zh/themes/yun/layout#首页标语
         原文要点：banner.title 支持手动分割成多段做交错排列文字效果
      3. MDN：prefers-reduced-motion
         https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion
         要点：动效必须尊重用户的减弱动效偏好，否则对前庭功能障碍用户不友好。
    原型结论：
      1. 文字入场用 CSS animation + 递增 delay 即可，比 JS 逐字快且省资源；
      2. 动效必须能用系统设置关闭。
    本处变体：
      - 原首页是「Hero + 卡片网格」，无动画；
      - Valaxy 用 framework 层的 banner 配置，本站直接写 CSS。
    沿用结论：保留大标题区（这是站点门面），加交错入场动画；
              文章列表改为横向卡片（更接近 Valaxy 的列表形态）。
    偏离处：
      - 不做「流动云」背景动画（Valaxy Yun 主题特有，与本项目星空背景冲突）；
      - 不做 i18n 多语言标题（本站为中文单语言）。
-->
<template>
  <div class="home">
    <!-- ===== 首屏：站点名与标语 ===== -->
    <section class="hero">
      <p class="hero-eyebrow">WELCOME TO MY SITE</p>

      <!-- 逐字入场：每个字一个 span，animation-delay 递增 -->
      <h1 class="hero-title" aria-label="飼酒之心">
        <span
          v-for="(ch, i) in titleChars"
          :key="i"
          class="hero-char"
          :style="{ animationDelay: `${i * 0.08}s` }"
        >{{ ch }}</span>
      </h1>

      <p class="hero-desc">
        用代码创造有趣的东西。这里记录了一些技术思考与随笔。
      </p>

      <p class="hero-contact">
        <span>{{ contact.wechat }}</span>
        <span class="dot">·</span>
        <span>{{ contact.qq }}</span>
      </p>

      <!-- 打赏码：点击展开，不占首屏高度 -->
      <div class="pay-row">
        <el-button
          v-for="q in payCodes"
          :key="q.key"
          size="small"
          text
          class="pay-chip"
          @click="activePay = q"
        >
          {{ q.label }}
        </el-button>
      </div>

      <!-- 向下箭头：提示可滚动 -->
      <div class="hero-scroll" aria-hidden="true">
        <span class="scroll-arrow"></span>
      </div>
    </section>

    <!-- 码图预览 -->
    <el-dialog
      v-model="payVisible"
      :title="activePay?.label"
      width="320px"
      align-center
      class="pay-dialog"
    >
      <img v-if="activePay" :src="activePay.src" :alt="activePay.label" class="pay-img" />
    </el-dialog>

    <!-- ===== 文章列表 ===== -->
    <section class="posts">
      <header class="posts-head">
        <h2 class="posts-title">最新文章</h2>
        <router-link to="/blog" class="posts-more">全部文章 →</router-link>
      </header>

      <!-- 加载中：骨架屏而非"加载中"文字 -->
      <div v-if="loading" class="post-list">
        <div v-for="i in 3" :key="i" class="post-skeleton">
          <el-skeleton :rows="2" animated />
        </div>
      </div>

      <!-- 空态 -->
      <div v-else-if="!articles.length" class="posts-empty">
        <el-empty description="还没有文章，去后台写一篇吧">
          <el-button v-if="isAdmin" type="primary" @click="$router.push('/admin')">
            去后台
          </el-button>
        </el-empty>
      </div>

      <!-- 文章卡片列表 -->
      <div v-else class="post-list">
        <router-link
          v-for="(a, i) in articles"
          :key="a.slug"
          :to="`/blog/${encodeURIComponent(a.slug)}`"
          class="post-card"
          :style="{ animationDelay: `${0.1 + i * 0.07}s` }"
        >
          <div v-if="a.cover_image" class="post-cover">
            <el-image :src="a.cover_image" :alt="a.title" fit="cover" lazy />
          </div>

          <div class="post-body">
            <div class="post-meta">
              <el-tag v-if="a.category" size="small" type="info" effect="plain">
                {{ categoryLabel(a.category) }}
              </el-tag>
              <span class="post-date">{{ formatDate(a.created_at) }}</span>
            </div>

            <h3 class="post-title">{{ a.title }}</h3>

            <p class="post-summary">{{ a.summary || excerpt(a) }}</p>

            <div v-if="a.tags && a.tags.length" class="post-tags">
              <el-tag
                v-for="t in a.tags"
                :key="t"
                size="small"
                effect="plain"
                class="post-tag"
              ># {{ t }}</el-tag>
            </div>
          </div>
        </router-link>
      </div>
    </section>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import http from '../api'
import { auth } from '../auth'
import { categoryLabel, formatDate } from '../labels'

const articles = ref([])
const loading = ref(true)
const activePay = ref(null)
const payVisible = ref(false)

const isAdmin = computed(() => auth.isAdmin)

/** 站点名逐字拆分（用于交错入场动画） */
const TITLE = '飼酒之心'
const titleChars = computed(() => TITLE.split(''))

/**
 * 联系方式
 *
 * 说明：从原 hardcode 保留，后续可移到后端配置。
 */
const contact = {
  wechat: '微信：hao202305',
  qq: 'QQ：1759737638',
}

/**
 * 打赏码图
 *
 * 图片放在 frontend/public/pay/，由 Vite 原样发布到站点根。
 * 不用 /static/ 前缀，因为那是后端图床目录。
 */
const payCodes = [
  { key: 'wechat-id', label: '微信号', src: '/pay/wechat-id.jpg' },
  { key: 'wechat-pay', label: '微信收款码', src: '/pay/wechat-pay.jpg' },
  { key: 'reward', label: '赞赏码', src: '/pay/reward.jpg' },
]

function openPay(code) {
  activePay.value = code
  payVisible.value = true
}

/** 去掉 Markdown 标记，取纯文本摘要 */
function excerpt(a) {
  const raw = a.content || ''
  if (!raw) return ''
  return raw
    .replace(/```[\s\S]*?```/g, '')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/[#>*_`~-]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 120)
}

onMounted(async () => {
  try {
    // 后端 /api/articles 返回纯数组（ListOut 序列化）
    const list = await http.get('/api/articles', { params: { limit: 6 } })
    articles.value = Array.isArray(list) ? list : []
  } catch (e) {
    console.error('加载文章失败:', e)
    articles.value = []
  } finally {
    loading.value = false
  }
})
</script>

<style scoped>
/* ==================== 首屏 ==================== */
.hero {
  min-height: 72vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 4rem 1rem 3rem;
  position: relative;
}

.hero-eyebrow {
  font-family: var(--mono, monospace);
  font-size: 0.7rem;
  letter-spacing: 0.34em;
  color: var(--accent, #4cc9f0);
  opacity: 0.75;
  margin-bottom: 1.4rem;
  animation: fadeUp 0.6s ease both;
}

/* 标题：逐字入场 */
.hero-title {
  font-size: clamp(2.6rem, 8vw, 4.4rem);
  font-weight: 700;
  letter-spacing: 0.06em;
  line-height: 1.15;
  margin-bottom: 1.6rem;
  display: flex;
  justify-content: center;
  flex-wrap: wrap;
}

.hero-char {
  display: inline-block;
  background: linear-gradient(135deg, #ffffff 10%, var(--accent, #4cc9f0) 55%, #7b6cf6 100%);
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  animation: charIn 0.7s cubic-bezier(0.2, 0.8, 0.3, 1) both;
  will-change: transform, opacity;
}

@keyframes charIn {
  from {
    opacity: 0;
    transform: translateY(0.5em) scale(0.92);
    filter: blur(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
    filter: blur(0);
  }
}

@keyframes fadeUp {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

.hero-desc {
  font-size: 1rem;
  color: var(--muted, #8b93a1);
  max-width: 34rem;
  line-height: 1.9;
  margin-bottom: 1.1rem;
  animation: fadeUp 0.7s 0.5s ease both;
}

.hero-contact {
  font-family: var(--mono, monospace);
  font-size: 0.8rem;
  color: var(--muted, #8b93a1);
  opacity: 0.8;
  display: flex;
  gap: 0.6rem;
  align-items: center;
  flex-wrap: wrap;
  justify-content: center;
  animation: fadeUp 0.7s 0.62s ease both;
}

.dot { opacity: 0.45; }

.pay-row {
  display: flex;
  gap: 0.4rem;
  margin-top: 1.4rem;
  flex-wrap: wrap;
  justify-content: center;
  animation: fadeUp 0.7s 0.74s ease both;
}

.pay-chip {
  font-size: 0.76rem !important;
  color: var(--muted, #8b93a1) !important;
}

.pay-chip:hover {
  color: var(--accent, #4cc9f0) !important;
}

/* 向下滚动提示 */
.hero-scroll {
  margin-top: 2.6rem;
  height: 34px;
  display: flex;
  justify-content: center;
  animation: fadeUp 0.7s 0.9s ease both;
}

.scroll-arrow {
  width: 1px;
  height: 32px;
  background: linear-gradient(to bottom, var(--accent, #4cc9f0), transparent);
  position: relative;
}

.scroll-arrow::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: -3px;
  width: 7px;
  height: 7px;
  border-right: 1px solid var(--accent, #4cc9f0);
  border-bottom: 1px solid var(--accent, #4cc9f0);
  transform: rotate(45deg);
  animation: scrollHint 1.8s ease-in-out infinite;
}

@keyframes scrollHint {
  0%, 100% { opacity: 0.3; transform: translateY(-4px) rotate(45deg); }
  50% { opacity: 1; transform: translateY(2px) rotate(45deg); }
}

/* ==================== 文章列表 ==================== */
.posts {
  max-width: 760px;
  margin: 0 auto;
  padding: 0 1.2rem 4rem;
}

.posts-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 1.4rem;
  padding-bottom: 0.8rem;
  border-bottom: 1px solid var(--border, #23262f);
}

.posts-title {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--text, #e8ecf1);
  position: relative;
  padding-left: 0.7rem;
}

/* 标题左侧的竖条，呼应 Valaxy 的分节视觉 */
.posts-title::before {
  content: '';
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 3px;
  height: 0.95em;
  border-radius: 2px;
  background: var(--accent, #4cc9f0);
}

.posts-more {
  font-size: 0.8rem;
  color: var(--muted, #8b93a1);
  transition: color 0.18s ease;
}

.posts-more:hover {
  color: var(--accent, #4cc9f0);
}

.post-list {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.post-skeleton {
  padding: 1.2rem;
  border: 1px solid var(--border, #23262f);
  border-radius: 12px;
  background: var(--surface, #111318);
}

/* 卡片：参考 Valaxy 的横向条目形态 */
.post-card {
  display: flex;
  gap: 1.1rem;
  padding: 1.1rem 1.2rem;
  border: 1px solid var(--border, #23262f);
  border-radius: 12px;
  background: var(--surface, #111318);
  color: inherit;
  text-decoration: none;
  transition:
    border-color 0.2s ease,
    transform 0.2s ease,
    box-shadow 0.2s ease;
  animation: cardIn 0.5s ease both;
  will-change: transform;
}

.post-card:hover {
  border-color: rgba(76, 201, 240, 0.42);
  transform: translateY(-2px);
  box-shadow: 0 8px 26px rgba(0, 0, 0, 0.28);
}

@keyframes cardIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.post-cover {
  flex: 0 0 132px;
  height: 92px;
  border-radius: 8px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.03);
}

.post-cover :deep(img) {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.post-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.post-meta {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}

.post-date {
  font-family: var(--mono, monospace);
  font-size: 0.72rem;
  color: var(--muted, #8b93a1);
  opacity: 0.85;
}

.post-title {
  font-size: 1.05rem;
  font-weight: 600;
  color: var(--text, #e8ecf1);
  line-height: 1.45;
  transition: color 0.18s ease;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.post-card:hover .post-title {
  color: var(--accent, #4cc9f0);
}

.post-summary {
  font-size: 0.85rem;
  color: var(--muted, #8b93a1);
  line-height: 1.72;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.post-tags {
  display: flex;
  gap: 0.35rem;
  flex-wrap: wrap;
  margin-top: 0.1rem;
}

.post-tag {
  font-family: var(--mono, monospace);
  font-size: 0.7rem !important;
}

.posts-empty {
  padding: 2.5rem 0;
}

/* ==================== 弹窗图片 ==================== */
.pay-img {
  width: 100%;
  border-radius: 8px;
  display: block;
}

/* ==================== 响应式 ==================== */
@media (max-width: 600px) {
  .hero {
    min-height: 62vh;
    padding: 3rem 1rem 2rem;
  }

  .post-card {
    flex-direction: column;
  }

  .post-cover {
    flex: none;
    width: 100%;
    height: 140px;
  }
}

/* ==================== 动效可访问性 ==================== */
/* 用户开启「减弱动效」时，全部动画立即完成，不做位移动画 */
@media (prefers-reduced-motion: reduce) {
  .hero-char,
  .hero-eyebrow,
  .hero-desc,
  .hero-contact,
  .pay-row,
  .hero-scroll,
  .post-card {
    animation: none !important;
  }

  .scroll-arrow::after {
    animation: none !important;
  }

  .post-card:hover {
    transform: none;
  }
}
</style>
