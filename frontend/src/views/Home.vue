<!--
  首页（结构逐行对照 YunYouJun/valaxy 的 PressHomeHero.vue）

  逻辑描述：
    输入：最新文章列表
    输出：
      1. press-home-hero：双栏 Grid（1.08fr / 1fr），左文右视觉
      2. press-home-features：特性卡片区（对照 PressHomeFeatures.vue）
      3. 文章列表（对照 PressArticleCard.vue 的条目形态）
    关键机制：
      1. 入场动画用上游的 .press-enter + --pr-enter-order 错峰系统，
         不再自己写逐字动画；
      2. 布局尺寸全部用上游的真实值：
           min-height: 620px
           padding: clamp(32px, 5vw, 72px)
           font-size: clamp(40px, 4.5vw, 68px)
           --pr-home-max-width: 1320px
      3. hero 结构分 eyebrow / heading(name+text) / tagline / actions / command，
         与上游一一对应。
    不变量：
      - 所有颜色来自 --press-c-* / --pr-* 变量；
      - 无 JS 时内容完全可见（动画只影响 opacity 起始值）。
    边界：
      - 窄屏（<900px）Hero 退化为单栏（上游同款断点）
      - 用户偏好减弱动效时不做任何动画

  依据：
    经典问题：设计系统的忠实移植（design system porting）
    来源：
      YunYouJun/valaxy · packages/valaxy-theme-press/components/PressHomeHero.vue
      https://github.com/YunYouJun/valaxy/blob/main/packages/valaxy-theme-press/components/PressHomeHero.vue
      关键结构（照抄）：
        <section class="press-home-hero">
          <div class="hero-copy">
            <p class="hero-eyebrow press-enter">{{ hero.eyebrow }}</p>
            <h1 class="hero-heading press-enter" :style="{ '--pr-enter-order': 1 }">
              <span class="hero-name">{{ hero.name }}</span>
              <span class="hero-text">{{ hero.text }}</span>
            </h1>
            <p class="hero-tagline press-enter" :style="{ '--pr-enter-order': 2 }">
            <div class="hero-actions press-enter" :style="{ '--pr-enter-order': 3 }">
          </div>
          <div class="hero-visual"> ... </div>
        </section>
      关键样式（照抄）：
        .press-home-hero {
          display: grid;
          grid-template-columns: minmax(0, 1.08fr) minmax(0, 1fr);
          border-bottom: 1px solid var(--pr-c-divider-light);
          min-height: 620px;
        }
        .hero-copy { padding: clamp(32px, 5vw, 72px); }
        .hero-name { color: var(--pr-c-brand); font-size: 16px; letter-spacing: .18em; }
        .hero-text { font-size: clamp(40px, 4.5vw, 68px); letter-spacing: -.045em; }
    原型结论：
      Valaxy 用「品牌色小字 + 超大主标题」的两级结构，
      比单纯一个大标题更有层次；入场用统一错峰而非逐字。
    本处变体：
      - hero 内容来自静态配置而非 frontmatter（本站没有 SSG）；
      - 右侧视觉用文章统计替代上游的 orbit 动画。
    沿用结论：结构、尺寸、断点、错峰入场全部照抄。
    偏离处：
      - 未实现 PressHomeOrbit（需要独立 canvas 动画）；
        以统计环替代，保持"右侧有视觉"这个布局关系。
-->
<template>
  <div class="press-home">
    <!-- ==================== Hero ==================== -->
    <section class="press-home-hero">
      <div class="hero-copy">
        <p class="hero-eyebrow press-enter">
          {{ hero.eyebrow }}
        </p>

        <h1 class="hero-heading press-enter" :style="{ '--pr-enter-order': 1 }">
          <span class="hero-name">{{ hero.name }}</span>
          <span class="hero-text">{{ hero.text }}</span>
        </h1>

        <p class="hero-tagline press-enter" :style="{ '--pr-enter-order': 2 }">
          {{ hero.tagline }}
        </p>

        <div class="hero-actions press-enter" :style="{ '--pr-enter-order': 3 }">
          <button class="press-btn press-btn--brand" @click="$router.push('/blog')">
            {{ hero.actions[0].text }}
          </button>
          <button class="press-btn press-btn--alt" @click="$router.push('/community')">
            {{ hero.actions[1].text }}
          </button>
        </div>

        <!-- 对应上游的 PressHomeCommand：一行终端提示 -->
        <div class="hero-command press-enter" :style="{ '--pr-enter-order': 4 }">
          <span class="cmd-prompt">$</span>
          <span class="cmd-text">{{ hero.command }}</span>
        </div>
      </div>

      <div class="hero-visual">
        <div class="hero-orbit">
          <div class="orbit-ring orbit-ring--1"></div>
          <div class="orbit-ring orbit-ring--2"></div>
          <div class="orbit-core">
            <span class="orbit-num">{{ articles.length }}</span>
            <span class="orbit-label">篇文章</span>
          </div>
        </div>

        <p v-if="latest" class="hero-caption">
          最新 · {{ latest.title }}
        </p>
      </div>
    </section>

    <!-- ==================== 特性区（对照 PressHomeFeatures.vue） ==================== -->
    <section class="press-home-features">
      <div
        v-for="(f, i) in features"
        :key="f.title"
        class="feature-item press-reveal"
        :style="{ '--pr-enter-order': i }"
      >
        <span class="feature-icon">{{ f.icon }}</span>
        <h3 class="feature-title">{{ f.title }}</h3>
        <p class="feature-detail">{{ f.detail }}</p>
      </div>
    </section>

    <!-- ==================== 文章列表 ==================== -->
    <section class="press-posts">
      <header class="posts-header">
        <h2 class="posts-title">最新文章</h2>
        <router-link to="/blog" class="posts-all">全部文章 →</router-link>
      </header>

      <div v-if="loading" class="posts-list">
        <div v-for="i in 3" :key="i" class="post-item post-item--skeleton">
          <el-skeleton :rows="2" animated />
        </div>
      </div>

      <el-empty v-else-if="!articles.length" description="还没有文章">
        <button v-if="isAdmin" class="press-btn press-btn--brand" @click="$router.push('/admin')">
          去后台写一篇
        </button>
      </el-empty>

      <div v-else class="posts-list">
        <router-link
          v-for="a in articles"
          :key="a.slug"
          :to="`/blog/${encodeURIComponent(a.slug)}`"
          class="post-item press-reveal"
        >
          <div class="post-main">
            <h3 class="post-title">{{ a.title }}</h3>
            <p class="post-summary">{{ a.summary || excerpt(a) }}</p>
          </div>
          <div class="post-side">
            <span v-if="a.category" class="post-cat">{{ categoryLabel(a.category) }}</span>
            <time class="post-time">{{ formatDate(a.created_at) }}</time>
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

const isAdmin = computed(() => auth.isAdmin)
const latest = computed(() => articles.value[0] || null)

/**
 * Hero 内容
 *
 * 对应 Valaxy 的 frontmatter.hero：
 *   eyebrow / name / text / tagline / actions / command
 * 本站没有 SSG，内容直接写在这里。
 */
const hero = {
  eyebrow: 'A PERSONAL SITE FOR EXPRESSION',
  name: 'WANLE',
  text: '飼酒之心',
  tagline: '用代码创造有趣的东西',
  actions: [
    { text: '阅读文章', link: '/blog', theme: 'brand' },
    { text: '逛逛社区', link: '/community', theme: 'alt' },
  ],
  command: 'write something worth reading',
}

/** 特性区（对应 PressHomeFeatures 的三项） */
const features = [
  { icon: '✍', title: '开箱即用', detail: '文章、分类、标签与归档已就绪，用 Markdown 写下第一篇。' },
  { icon: '⚡', title: '即改即见', detail: '保存后立刻可读，写作与预览之间没有等待。' },
  { icon: '🎨', title: '随心定制', detail: '从页脚到布局，每一处都能改成你想要的样子。' },
]

/** 纯文本摘要 */
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
    .slice(0, 110)
}

onMounted(async () => {
  try {
    const list = await http.get('/api/articles', { params: { limit: 6 } })
    articles.value = Array.isArray(list) ? list : []
  } catch (e) {
    console.error('加载文章失败:', e)
  } finally {
    loading.value = false
  }
})
</script>

<style scoped>
/* ==================== 容器（照抄 .press-home） ==================== */
.press-home {
  width: min(100% - 48px, var(--pr-home-max-width));
  margin-inline: auto;
  border-inline: 1px solid var(--pr-c-divider-light);
}

/* ==================== Hero（照抄 .press-home-hero） ==================== */
.press-home-hero {
  display: grid;
  grid-template-columns: minmax(0, 1.08fr) minmax(0, 1fr);
  border-bottom: 1px solid var(--pr-c-divider-light);
  min-height: 620px;
}

.hero-copy {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  padding: clamp(32px, 5vw, 72px);
  min-width: 0;
}

.hero-eyebrow {
  margin: 0 0 24px;
  color: var(--press-c-text-2);
  font-size: 12px;
  font-family: var(--pr-font-mono);
  letter-spacing: 0.08em;
}

.hero-heading {
  margin: 0;
  font-family: var(--pr-font-display);
  font-weight: 600;
}

.hero-name {
  display: block;
  margin-bottom: 20px;
  color: var(--pr-c-brand);
  font-size: 16px;
  font-weight: 600;
  font-family: var(--pr-font-mono);
  letter-spacing: 0.18em;
}

.hero-text {
  display: block;
  font-size: clamp(40px, 4.5vw, 68px);
  letter-spacing: -0.045em;
  line-height: 1.12;
  overflow-wrap: anywhere;
  text-wrap: balance;
}

.hero-tagline {
  margin: 24px 0 0;
  color: var(--press-c-text-2);
  font-size: clamp(15px, 1.5vw, 18px);
  line-height: 1.6;
  max-width: 32em;
}

.hero-actions {
  display: flex;
  gap: 12px;
  margin-top: 36px;
  flex-wrap: wrap;
}

/* 按钮：照抄上游的 --pr-button-brand-* 变量 */
.press-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 20px;
  height: 44px;
  border-radius: 8px;
  border: 1px solid var(--pr-c-divider-light);
  background: transparent;
  color: var(--press-c-text-1);
  font-family: inherit;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition:
    background-color 0.2s var(--pr-ease-standard),
    border-color 0.2s var(--pr-ease-standard),
    color 0.2s var(--pr-ease-standard),
    box-shadow 0.2s var(--pr-ease-standard);
}

.press-btn:hover {
  border-color: var(--pr-c-brand);
  color: var(--pr-c-brand);
}

.press-btn--brand {
  background: var(--pr-button-brand-bg);
  border-color: transparent;
  color: #fff;
  box-shadow: 0 2px 12px var(--pr-button-brand-shadow);
}

.press-btn--brand:hover {
  color: #fff;
  box-shadow: 0 4px 18px var(--pr-button-brand-shadow-hover);
}

/* 命令提示（对应 PressHomeCommand） */
.hero-command {
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  max-width: 380px;
  margin-top: 36px;
  padding: 12px 14px;
  border: 1px solid var(--pr-c-divider-light);
  border-radius: 8px;
  background: var(--pr-c-surface);
  font-family: var(--pr-font-mono);
  font-size: 13px;
}

.cmd-prompt {
  color: var(--pr-c-brand);
}

.cmd-text {
  color: var(--press-c-text-2);
}

/* ---------- 右侧视觉 ---------- */
.hero-visual {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 20px;
  padding: clamp(32px, 5vw, 72px);
  border-left: 1px solid var(--pr-c-divider-light);
  min-width: 0;
}

.hero-orbit {
  position: relative;
  width: 100%;
  max-width: 280px;
  aspect-ratio: 1;
  display: grid;
  place-items: center;
}

.orbit-ring {
  position: absolute;
  border-radius: 50%;
  border: 1px solid var(--pr-c-orbit-line);
}

.orbit-ring--1 {
  inset: 0;
  border-style: dashed;
  animation: spin 40s linear infinite;
}

.orbit-ring--2 {
  inset: 16%;
  box-shadow: 0 0 40px var(--pr-c-orbit-glow) inset;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.orbit-core {
  display: flex;
  flex-direction: column;
  align-items: center;
  z-index: 1;
}

.orbit-num {
  font-family: var(--pr-font-display);
  font-size: 3rem;
  font-weight: 600;
  line-height: 1;
  color: var(--press-c-text-1);
  font-variant-numeric: tabular-nums;
}

.orbit-label {
  margin-top: 8px;
  font-size: 12px;
  letter-spacing: 0.14em;
  color: var(--press-c-text-3);
}

.hero-caption {
  margin: 0;
  font-family: var(--pr-font-mono);
  font-size: 12px;
  color: var(--press-c-text-3);
  text-align: center;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ==================== 特性区 ==================== */
.press-home-features {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  border-bottom: 1px solid var(--pr-c-divider-light);
}

.feature-item {
  padding: clamp(24px, 3vw, 40px);
  border-right: 1px solid var(--pr-c-divider-light);
}

.feature-item:last-child {
  border-right: none;
}

.feature-icon {
  display: block;
  font-size: 20px;
  margin-bottom: 14px;
  color: var(--pr-c-brand);
}

.feature-title {
  margin: 0 0 8px;
  font-size: 15px;
  font-weight: 600;
  color: var(--press-c-text-1);
}

.feature-detail {
  margin: 0;
  font-size: 13px;
  line-height: 1.7;
  color: var(--press-c-text-2);
}

/* ==================== 文章列表 ==================== */
.press-posts {
  padding: clamp(32px, 4vw, 56px) clamp(16px, 3vw, 48px) 64px;
}

.posts-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  padding-bottom: 16px;
  margin-bottom: 4px;
  border-bottom: 1px solid var(--pr-c-divider-light);
}

.posts-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--press-c-text-1);
}

.posts-all {
  font-size: 13px;
  color: var(--press-c-text-3);
}

.posts-all:hover {
  color: var(--pr-c-brand);
}

.posts-list {
  display: flex;
  flex-direction: column;
}

/* 条目：只有分隔线（照抄 Valaxy 的列表语言） */
.post-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 32px;
  padding: 20px 4px;
  border-bottom: 1px solid var(--pr-c-divider-light);
  color: inherit;
  text-decoration: none;
  transition: background-color 0.2s var(--pr-ease-standard);
}

.post-item:hover {
  background: var(--pr-c-brand-soft);
}

.post-item--skeleton {
  display: block;
}

.post-main {
  flex: 1;
  min-width: 0;
}

.post-title {
  margin: 0 0 6px;
  font-size: 16px;
  font-weight: 600;
  line-height: 1.5;
  color: var(--press-c-text-1);
  transition: color 0.2s var(--pr-ease-standard);
}

.post-item:hover .post-title {
  color: var(--pr-c-brand);
}

.post-summary {
  margin: 0;
  font-size: 13.5px;
  line-height: 1.7;
  color: var(--press-c-text-2);
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.post-side {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 6px;
  flex-shrink: 0;
}

.post-cat {
  font-family: var(--pr-font-mono);
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--pr-c-brand-soft);
  color: var(--pr-c-brand);
}

.post-time {
  font-family: var(--pr-font-mono);
  font-size: 11.5px;
  color: var(--press-c-text-3);
}

/* ==================== 响应式（照抄上游断点） ==================== */
@media (max-width: 900px) {
  .press-home {
    width: min(100% - 32px, var(--pr-home-max-width));
    border-inline: none;
  }

  .press-home-hero {
    grid-template-columns: minmax(0, 1fr);
    min-height: auto;
  }

  .hero-visual {
    border-left: none;
    border-top: 1px solid var(--pr-c-divider-light);
  }

  .press-home-features {
    grid-template-columns: minmax(0, 1fr);
  }

  .feature-item {
    border-right: none;
    border-bottom: 1px solid var(--pr-c-divider-light);
  }

  .feature-item:last-child {
    border-bottom: none;
  }
}

@media (max-width: 600px) {
  .post-item {
    flex-direction: column;
    gap: 10px;
  }

  .post-side {
    flex-direction: row;
    align-items: center;
    gap: 10px;
  }
}

/* 减弱动效 */
@media (prefers-reduced-motion: reduce) {
  .orbit-ring--1 {
    animation: none;
  }
}
</style>
