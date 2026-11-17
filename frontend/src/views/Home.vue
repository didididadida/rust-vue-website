<template>
  <div class="home">
    <!-- Hero 区域：只留站名与联系方式，不放任何按钮 -->
    <section class="hero">
      <p class="hero-eyebrow">BLOG</p>
      <h1 class="hero-title">wanle</h1>
      <p class="hero-desc">
        用代码创造有趣的东西。这里记录了一些技术思考与随笔。
      </p>
      <p class="hero-contact">
        <span>微信：your-wechat</span>
        <span class="dot">·</span>
        <span>QQ：your-qq</span>
      </p>
      <!-- 三张码图：点击展开，不占首屏高度（渐进披露） -->
      <div class="pay-row">
        <button
          v-for="q in payCodes"
          :key="q.key"
          type="button"
          class="pay-chip"
          @click="openPay(q)"
        >
          {{ q.label }}
        </button>
      </div>
      <div class="hero-rule"></div>
    </section>

    <!-- 码图预览层 -->
    <div v-if="activePay" class="pay-overlay" @click.self="activePay = null">
      <div class="pay-card">
        <p class="pay-title">{{ activePay.label }}</p>
        <img :src="activePay.src" :alt="activePay.label" class="pay-img" />
        <button type="button" class="pay-close" @click="activePay = null">关闭</button>
      </div>
    </div>

    <!-- 最新文章 -->
    <section class="section">
      <div class="section-header">
        <h2 class="section-title">最新文章</h2>
      </div>

      <div v-if="loading.articles" class="loading">加载中</div>
      <div v-else-if="articles.length === 0" class="empty">
        <p>还没有文章，快去后台写一篇吧</p>
      </div>
      <div v-else class="card-grid">
        <article v-for="item in articles" :key="item.slug" class="card article-card">
          <router-link
            :to="'/blog/' + encodeURIComponent(item.slug)"
            class="card-link"
            :aria-label="item.title"
          >
            <div v-if="item.cover_image" class="card-cover">
              <img :src="item.cover_image" :alt="item.title" loading="lazy" />
            </div>
            <div class="card-body">
              <div class="card-meta">
                <span class="category-badge">{{ categoryLabel(item.category) }}</span>
                <span class="dot">·</span>
                <span class="date">{{ formatDate(item.created_at) }}</span>
              </div>
              <h3 class="card-title">{{ item.title }}</h3>
              <p class="card-summary">{{ excerpt(item) }}</p>
              <div v-if="item.tags && item.tags.length" class="card-tags">
                <span v-for="tag in item.tags" :key="tag" class="tag">{{ tag }}</span>
              </div>
            </div>
          </router-link>
        </article>
      </div>
    </section>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import http from '../api'
import { categoryLabel, formatDate } from '../labels'

const articles = ref([])
const loading = ref({ articles: true })

/**
 * 联系与打赏码图。
 * 图片放在 frontend/public/pay/，由 Vite 原样发布到站点根，故路径不以 /static/ 开头
 * （/static/ 是后端图床目录，受后端鉴权与缓存策略管辖，不应塞站点自有的静态资源）。
 */
const payCodes = [
  { key: 'wechat-id', label: '微信号', src: '/pay/wechat-id.jpg' },
  { key: 'wechat-pay', label: '微信收款码', src: '/pay/wechat-pay.jpg' },
  { key: 'reward', label: '赞赏码', src: '/pay/reward.jpg' },
]
const activePay = ref(null)
function openPay(code) {
  activePay.value = code
}

/** 摘要：优先用 summary，否则从正文剥掉 Markdown 标记截取 */
const stripMarkdown = text =>
  String(text || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/[#>*_`~\[\]()!]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()

const excerpt = item => {
  if (item.summary) return item.summary
  const text = stripMarkdown(item.content)
  return text.length > 110 ? text.slice(0, 110) + '…' : text
}

onMounted(async () => {
  try {
    const latestArticles = await http.get('/api/articles', { params: { limit: 3 } })
    articles.value = latestArticles
  } catch (e) {
    console.error('加载数据失败:', e)
  } finally {
    loading.value = { articles: false }
  }
})
</script>

<style scoped>
/* ---------- 码图入口 ---------- */
.pay-row {
  display: flex;
  justify-content: center;
  flex-wrap: wrap;
  gap: 0.6rem;
  margin-top: 1.1rem;
}
.pay-chip {
  font-size: 0.8rem;
  padding: 0.3rem 0.85rem;
  border-radius: 999px;
  border: 1px solid var(--dsw-alias-border-subtle, rgba(255, 255, 255, 0.16));
  background: transparent;
  color: var(--dsw-alias-label-secondary, #b8c0cc);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.pay-chip:hover {
  background: rgba(255, 255, 255, 0.06);
  color: var(--dsw-alias-label-primary, #e8edf5);
}

.pay-overlay {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.62);
  backdrop-filter: blur(2px);
  padding: 1.5rem;
}
.pay-card {
  background: var(--dsw-alias-bg-elevated, #151a22);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 14px;
  padding: 1.1rem 1.1rem 0.9rem;
  text-align: center;
  max-width: 320px;
}
.pay-title {
  margin: 0 0 0.7rem;
  font-size: 0.9rem;
  color: var(--dsw-alias-label-secondary, #b8c0cc);
}
/* 固定的正方形容器：码图尺寸不一，避免弹窗跳动 */
.pay-img {
  width: 260px;
  height: 260px;
  object-fit: contain;
  border-radius: 8px;
  background: #fff;
}
.pay-close {
  margin-top: 0.8rem;
  font-size: 0.8rem;
  padding: 0.3rem 1.1rem;
  border-radius: 999px;
  border: 1px solid rgba(255, 255, 255, 0.16);
  background: transparent;
  color: var(--dsw-alias-label-secondary, #b8c0cc);
  cursor: pointer;
}

/* ---------- Hero ---------- */
.hero {
  text-align: center;
  padding: 5.5rem 1rem 4rem;
  position: relative;
}

/* 英文小标：等宽字体 + 字距展开，像飞船编号 */
.hero-eyebrow {
  display: inline-block;
  font-family: var(--font-mono);
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.38em;
  color: var(--accent);
  margin-bottom: 1.6rem;
  padding: 0.32rem 1.1rem 0.32rem 1.4rem;
  border: 1px solid rgba(76, 201, 240, 0.24);
  border-radius: var(--radius-full);
  background: rgba(76, 201, 240, 0.05);
  opacity: 0.9;
}

.hero-title {
  font-family: var(--font-serif);
  font-size: clamp(2.5rem, 7.5vw, 4rem);
  font-weight: 600;
  line-height: 1.2;
  letter-spacing: 0.06em;
  margin-bottom: 1.6rem;
  /* 冷白 → 冰蓝 → 星云紫 的静态渐变
     性能：原来这里是 background-position 无限动画 + filter: drop-shadow，
     两者叠加会让标题每帧都重绘并重新计算模糊，是大标题上的主要开销。
     现在改为静态渐变 + 普通 text-shadow（一次绘制即可）。 */
  background: linear-gradient(
    100deg,
    #ffffff 0%,
    #cfe9ff 25%,
    var(--accent) 55%,
    var(--nebula-soft) 85%
  );
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
}

.hero-desc {
  font-size: 1.02rem;
  color: var(--text-secondary);
  max-width: 35rem;
  margin: 0 auto 1rem;
  line-height: 1.95;
  letter-spacing: 0.02em;
}

/* 联系方式：站名下方的小字，等宽 + 低饱和，不与内容抢注意力 */
.hero-contact {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin: 0 auto 2.6rem;
  font-family: var(--font-mono);
  font-size: 0.8rem;
  letter-spacing: 0.06em;
  color: var(--text-muted);
}

/* 分隔：一条渐隐的轨道线，中间一颗呼吸的星 */
.hero-rule {
  position: relative;
  width: 220px;
  height: 1px;
  margin: 4.2rem auto 0;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(126, 156, 210, 0.25),
    transparent
  );
}

.hero-rule::after {
  content: '';
  position: absolute;
  left: 50%;
  top: 50%;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--accent);
  transform: translate(-50%, -50%);
  animation: starPulse 3.6s ease-in-out infinite;
}

/* ---------- 区块 ---------- */
.section {
  margin-top: 4.5rem;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1.5rem;
  gap: 1rem;
}

.section-title {
  font-family: var(--font-serif);
  font-size: 1.32rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  color: #eef4ff;
  padding-left: 0.9rem;
  position: relative;
}

/* 左侧冷光竖条 */
.section-title::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0.24em;
  bottom: 0.24em;
  width: 2px;
  border-radius: var(--radius-full);
  background: linear-gradient(180deg, var(--accent), rgba(76, 201, 240, 0));
  box-shadow: 0 0 10px rgba(76, 201, 240, 0.6);
}

/* ---------- 卡片 ---------- */
.article-card {
  padding: 0;
  display: flex;
}

.toy-card,
.tool-card {
  padding: 0;
}

.card-link {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  color: inherit;
  text-decoration: none;
}

.article-card:hover .card-title,
.toy-card:hover .card-title,
.tool-card:hover .card-title {
  color: var(--accent-soft);
}

.card-cover {
  width: 100%;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  background: var(--bg-overlay);
  position: relative;
}

/* 封面底部渐隐，让图片与卡片底色自然衔接 */
.card-cover::after {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, transparent 45%, rgba(10, 15, 29, 0.9));
  pointer-events: none;
}

.card-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.6s var(--ease-out), filter 0.4s var(--ease);
  filter: saturate(0.92);
}

.article-card:hover .card-cover img {
  transform: scale(1.05);
  filter: saturate(1.05) brightness(1.04);
}

.card-body {
  padding: 1.35rem 1.5rem 1.5rem;
  display: flex;
  flex-direction: column;
  flex: 1;
}

.card-meta {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.7rem;
  flex-wrap: wrap;
}

/* 分类：等宽小字，像一枚数据标签 */
.category-badge {
  font-family: var(--font-mono);
  font-size: 0.66rem;
  font-weight: 600;
  letter-spacing: 0.1em;
  color: var(--accent-soft);
  background: rgba(76, 201, 240, 0.09);
  padding: 0.14rem 0.5rem;
  border-radius: 4px;
  border: 1px solid rgba(76, 201, 240, 0.24);
}

.dot {
  color: var(--text-muted);
  font-size: 0.7rem;
  opacity: 0.6;
}

.date {
  font-family: var(--font-mono);
  font-size: 0.72rem;
  color: var(--text-muted);
  letter-spacing: 0.04em;
}

.card-title {
  font-family: var(--font-serif);
  font-size: 1.12rem;
  font-weight: 600;
  line-height: 1.58;
  margin-bottom: 0.62rem;
  color: #e8f0fc;
  transition: color 0.24s var(--ease);
}

.article-card:hover .card-title {
  color: var(--accent-soft);
}

.card-summary {
  font-size: 0.87rem;
  color: var(--text-secondary);
  line-height: 1.8;
  margin-bottom: 1rem;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.card-tags {
  display: flex;
  flex-wrap: wrap;
  margin-top: auto;
}

.tool-icon {
  font-size: 1.7rem;
  margin-bottom: 0.7rem;
  line-height: 1;
  filter: drop-shadow(0 0 12px rgba(76, 201, 240, 0.35));
}

.tool-card .card-body {
  padding: 1.5rem;
}

.tool-card .card-summary {
  margin-bottom: 1.1rem;
}

/* ---------- 响应式 ---------- */
@media (max-width: 860px) {
  .hero {
    padding: 2.75rem 0.5rem 2.25rem;
  }

  .hero-title {
    font-size: 2.3rem;
    letter-spacing: 0.04em;
  }

  .hero-eyebrow {
    font-size: 0.62rem;
    letter-spacing: 0.26em;
    padding-left: 1.1rem;
  }

  .hero-desc {
    font-size: 0.96rem;
  }

  .hero-rule {
    margin-top: 2.8rem;
    width: 150px;
  }

  .section {
    margin-top: 3.2rem;
  }

  .section-title {
    font-size: 1.18rem;
  }
}
</style>
