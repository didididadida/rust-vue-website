<!--
  首页 —— 对齐 didididadida.github.io 的 Yun 主题

  逻辑描述：
    输入：最新文章
    输出：分三段
      1. Yun Banner：竖排大字 + 上下线条展开动画（Yun 主题的标志性开场）
      2. 文章列表：Yun 的卡片式条目（标题 + 日期 + 字数 + 标签）
      3. 页脚信息
    关键机制：
      1. Banner 动画逐字复刻 Yun 主题的 banner.scss：
         - 上下两条 1px 竖线从中间向外展开（extend-line）
         - 展开后文字逐字从中间向两侧淡入（char-move-left / char-move-right）
         - 每个字有 hover 反白效果
      2. 竖排文字用 flex-column，每行两个字（左/右）向外展开
      3. 用户偏好减弱动效时直接显示静态结果
    不变量：
      - 无 JS 时文字已渲染（动画只控制 opacity 与 border 宽度）
      - 所有颜色来自 --yun-* / --va-c-* 变量
    边界：
      - 窄屏（<=768px）字号与间距缩小（上游 --rect-margin 同款断点）
      - 标题字数必须偶数，奇数时最后一行只有一个字（居中显示）

  依据：
    经典问题：设计系统的忠实移植（design system porting）
    来源：
      YunYouJun/valaxy 的 Yun 主题
      - styles/widgets/banner.scss —— 线条与字符动画的关键帧
        https://github.com/YunYouJun/valaxy/blob/main/packages/valaxy-theme-yun/styles/widgets/banner.scss
        关键：@keyframes extend-line / char-move-left / char-move-right
      - styles/css-vars.scss —— 颜色与尺寸变量
        关键：--banner-line-color / --banner-char-* / --yun-nav-height
      - components/YunBanner.vue —— 三段式状态机 enter → active → exit
      - 你的站点配置（site.config.ts）：title / subtitle / description / avatar
    原型结论：
      1. Yun 的开场动画是「线条先展开、文字后展开」的两阶段，
         不是同时发生——这是它区别于其他主题的地方；
      2. 文字是竖排的，每行两个字分别向左/右展开，形成"打开"的感觉；
      3. banner 高度是 100vh，占据整个首屏。
    本处变体：
      - 原主题用 useYunBanner composable 计算字符布局，本站直接写死布局；
      - 原主题有 nimbo/strato 两种模式（有无开场动画），本站实现 nimbo（带动画）。
    沿用结论：两阶段动画、竖排布局、hover 反白、100vh 首屏全部照抄。
    偏离处：
      - 未实现「重访时不重播动画」（原主题用 sessionStorage 判断，
        本站简化为每次都播放，用户可通过 prefers-reduced-motion 关闭）；
      - 云朵动画未实现（需要独立的 SVG 路径动画，见 YunCloud.vue）。
-->
<template>
  <div class="yun-home">
    <!-- ==================== Banner（Yun 开场动画） ==================== -->
    <section id="yun-banner" :class="{ 'is-intro-playing': introPlaying }">
      <!-- 上线条 -->
      <div class="banner-line-container">
        <div class="banner-line vertical-line-top" :class="lineStatus" />
      </div>

      <!-- 竖排大字 -->
      <div class="banner-char-container">
        <div
          v-for="(row, ri) in rows"
          :key="ri"
          class="banner-row"
        >
          <span
            v-for="(ch, ci) in row"
            :key="ci"
            class="char"
            :class="ci === 0 ? 'char-left' : 'char-right'"
            :style="{ animationDelay: `${ri * 0.12}s`, fontSize: charSize }"
          >{{ ch }}</span>
        </div>
      </div>

      <!-- 下线条 -->
      <div class="banner-line-container bottom">
        <div class="banner-line vertical-line-bottom" :class="lineStatus" />
      </div>

      <!-- 向下箭头 -->
      <div class="go-down" aria-hidden="true">
        <span class="go-down-inner"></span>
      </div>
    </section>

    <!-- ==================== 站点简介 ==================== -->
    <section class="site-intro">
      <img class="site-avatar" :src="site.avatar" :alt="site.author" />
      <h1 class="site-title">{{ site.title }}</h1>
      <p class="site-subtitle">{{ site.subtitle }}</p>
      <p class="site-desc">{{ site.description }}</p>
    </section>

    <!-- ==================== 文章列表 ==================== -->
    <section class="yun-posts">
      <header class="posts-header">
        <h2 class="posts-title">文章</h2>
        <router-link to="/blog" class="posts-all">全部 →</router-link>
      </header>

      <div v-if="loading" class="posts-list">
        <div v-for="i in 3" :key="i" class="post-card post-card--skeleton">
          <el-skeleton :rows="2" animated />
        </div>
      </div>

      <el-empty v-else-if="!articles.length" description="还没有文章" />

      <div v-else class="posts-list">
        <router-link
          v-for="a in articles"
          :key="a.slug"
          :to="`/blog/${encodeURIComponent(a.slug)}`"
          class="post-card"
        >
          <h3 class="post-card-title">{{ a.title }}</h3>
          <p class="post-card-summary">{{ a.summary || excerpt(a) }}</p>
          <div class="post-card-meta">
            <span class="meta-date">{{ formatDate(a.created_at) }}</span>
            <span class="meta-dot">·</span>
            <span class="meta-words">{{ wordCount(a) }} 字</span>
            <template v-if="a.tags && a.tags.length">
              <span v-for="t in a.tags" :key="t" class="meta-tag">#{{ t }}</span>
            </template>
          </div>
        </router-link>
      </div>
    </section>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import http from '../api'
import { formatDate } from '../labels'

const articles = ref([])
const loading = ref(true)

/**
 * 站点信息（照抄你 github.io 的 site.config.ts）
 */
const site = {
  title: '飼酒の小站',
  subtitle: 'All at sea.',
  description: 'please believe yourself',
  author: '飼酒の',
  avatar: 'https://s1.1128888.xyz/2024/08/08/66b3fd56df197.jpg',
}

/** Banner 文字（对应 themeConfig.banner.title） */
const BANNER_TITLE = '滴滴滴嗒嘀嗒'

/**
 * 每行两个字：Yun 的布局是左右各一个字，向两侧展开
 *
 * 奇数长度时最后一行只有一个字（放左侧）。
 */
const rows = computed(() => {
  const out = []
  for (let i = 0; i < BANNER_TITLE.length; i += 2) {
    out.push(BANNER_TITLE.slice(i, i + 2).split(''))
  }
  return out
})

/** 字号：随字数减少而变大（上游同样逻辑，字数少时字大） */
const charSize = computed(() => {
  const n = rows.value.length
  if (n <= 3) return '3.2rem'
  if (n <= 5) return '2.6rem'
  return '2rem'
})

/**
 * 线条动画状态：enter → active
 *
 * 照抄 YunBanner.vue 的状态机（简化版）：
 *   - 挂载后 500ms：线条从 0 展开到目标高度
 *   - 文字随后逐字淡入（由 CSS animation-delay 控制）
 */
const lineStatus = ref('')
const introPlaying = ref(false)

let timers = []

onMounted(() => {
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches

  if (reduced) {
    // 减弱动效：直接到最终状态
    lineStatus.value = 'active'
    return
  }

  introPlaying.value = true
  lineStatus.value = 'enter'

  timers.push(
    setTimeout(() => {
      lineStatus.value = 'active'
    }, 500)
  )

  timers.push(
    setTimeout(() => {
      introPlaying.value = false
    }, 1500)
  )
})

onBeforeUnmount(() => {
  timers.forEach(clearTimeout)
  timers = []
})

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
    .slice(0, 100)
}

/** 字数统计（中英文分开计，与 Valaxy 的 readTime 算法同思路） */
function wordCount(a) {
  const raw = (a.content || a.summary || '').replace(/```[\s\S]*?```/g, '')
  const cn = (raw.match(/[\u4e00-\u9fa5]/g) || []).length
  const en = (raw.match(/[a-zA-Z]+/g) || []).length
  return cn + en
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
/* ==================== Banner（照抄 banner.scss） ==================== */
#yun-banner {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  width: 100%;
  height: 100vh;
  --banner-line-height: 0;
  --line-animation-duration: 0.4s;
  --banner-char-size: 2.6rem;
  --banner-empty-border-size: 2.6rem;
}

.banner-line-container {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  height: var(--banner-line-height);
  transition: height 2s;
}

.banner-line-container.bottom {
  justify-content: end;
}

.vertical-line-top,
.vertical-line-bottom {
  display: flex;
  background-color: var(--banner-line-color, #fff);
  width: 1px;
  height: 0;
}

.vertical-line-bottom {
  flex-direction: column-reverse;
}

.banner-line.enter {
  animation-name: extend-line;
  animation-duration: var(--line-animation-duration);
  animation-fill-mode: forwards;
  animation-timing-function: cubic-bezier(0.18, 0.66, 0.05, 0.96);
  animation-iteration-count: 1;
}

.banner-line.active {
  height: var(--banner-line-height);
}

@keyframes extend-line {
  from {
    height: 0;
  }
  to {
    height: var(--banner-line-height);
  }
}

/* ---------- 字符容器 ---------- */
.banner-char-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.2rem;
  padding: 1.2rem 0;
}

.banner-row {
  display: flex;
  justify-content: center;
  gap: 0.35rem;
}

/* ---------- 单个字符（照抄 .char） ---------- */
.char {
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--va-font-serif, 'Noto Serif SC', serif);
  font-weight: 900;
  line-height: 1;
  color: var(--banner-char-color, #fff);
  background-color: var(--banner-char-bg-color, rgba(255, 255, 255, 0.5));
  opacity: 0;
  transition:
    color 0.3s ease-in-out,
    background-color 0.3s ease-in-out;
  padding: 0.08em 0.14em;
  cursor: default;
}

.char:hover {
  color: var(--banner-char-hover-color, #000);
  background-color: var(--banner-char-color, #fff);
}

/* 左字：向右展开 */
.char-left {
  border-left: 1px solid var(--banner-line-color, #fff);
  border-right: 0 solid rgba(76, 201, 240, 0.1);
  animation-name: char-move-left;
  animation-duration: 0.4s;
  animation-delay: calc(var(--line-animation-duration) + 0.1s);
  animation-fill-mode: forwards;
  animation-timing-function: ease-out;
}

/* 右字：向左展开 */
.char-right {
  border-right: 1px solid var(--banner-line-color, #fff);
  border-left: 0 solid rgba(76, 201, 240, 0.1);
  animation-name: char-move-right;
  animation-duration: 0.4s;
  animation-delay: calc(var(--line-animation-duration) + 0.1s);
  animation-fill-mode: forwards;
  animation-timing-function: ease-out;
}

@keyframes char-move-left {
  from {
    opacity: 0;
    border-right-width: 0;
  }
  to {
    opacity: 1;
    border-right-width: var(--banner-empty-border-size, 2.6rem);
  }
}

@keyframes char-move-right {
  from {
    opacity: 0;
    border-left-width: 0;
  }
  to {
    opacity: 1;
    border-left-width: var(--banner-empty-border-size, 2.6rem);
  }
}

/* ---------- 向下箭头 ---------- */
.go-down {
  position: absolute;
  bottom: 2rem;
  left: 50%;
  transform: translateX(-50%);
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  animation: float 2.2s ease-in-out infinite;
}

.go-down-inner {
  width: 10px;
  height: 10px;
  border-right: 1.5px solid var(--banner-char-color, #fff);
  border-bottom: 1.5px solid var(--banner-char-color, #fff);
  transform: rotate(45deg);
  opacity: 0.6;
}

@keyframes float {
  0%, 100% { transform: translateX(-50%) translateY(0); }
  50% { transform: translateX(-50%) translateY(6px); }
}

/* ==================== 站点简介 ==================== */
.site-intro {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 3rem 1.5rem 2rem;
  max-width: 900px;
  margin: 0 auto;
}

.site-avatar {
  width: 88px;
  height: 88px;
  border-radius: 50%;
  object-fit: cover;
  border: 2px solid var(--yun-surface-line, rgba(122, 122, 122, 0.15));
  margin-bottom: 1.2rem;
  transition: transform 0.3s ease;
}

.site-avatar:hover {
  transform: scale(1.05) rotate(4deg);
}

.site-title {
  font-size: 1.6rem;
  font-weight: 700;
  color: var(--va-c-text, #dfdfd6);
  margin-bottom: 0.5rem;
}

.site-subtitle {
  font-family: var(--va-font-serif, serif);
  font-size: 1rem;
  color: var(--va-c-text-2, #98989f);
  margin-bottom: 0.4rem;
  letter-spacing: 0.06em;
}

.site-desc {
  font-size: 0.88rem;
  color: var(--va-c-text-3, #6a6a71);
}

/* ==================== 文章卡片（照抄 Yun 的卡片形态） ==================== */
.yun-posts {
  max-width: 900px;
  margin: 0 auto;
  padding: 1rem 1.5rem 5rem;
}

.posts-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 1rem;
}

.posts-title {
  font-size: 1.15rem;
  font-weight: 600;
  color: var(--va-c-text, #dfdfd6);
}

.posts-all {
  font-size: 0.8rem;
  color: var(--va-c-text-3, #6a6a71);
}

.posts-all:hover {
  color: var(--va-c-link, #4cc9f0);
}

.posts-list {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

/* 卡片：圆角 + 悬停上浮（Yun 的 YunCard 风格） */
.post-card {
  display: block;
  padding: 1.25rem 1.4rem;
  border-radius: 12px;
  border: 1px solid var(--yun-surface-line, rgba(122, 122, 122, 0.12));
  background: var(--va-c-bg-soft, #121215);
  color: inherit;
  text-decoration: none;
  transition:
    transform 0.25s ease,
    border-color 0.25s ease,
    box-shadow 0.25s ease;
}

.post-card:hover {
  transform: translateY(-3px);
  border-color: var(--va-c-link, #4cc9f0);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
}

.post-card--skeleton {
  pointer-events: none;
}

/* 标题：渐变文字（Yun 的 gradient-text 效果） */
.post-card-title {
  font-size: 1.05rem;
  font-weight: 600;
  line-height: 1.5;
  margin-bottom: 0.4rem;
  background: linear-gradient(
    120deg,
    var(--va-c-text, #dfdfd6) 30%,
    var(--va-c-link, #4cc9f0)
  );
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
}

.post-card-summary {
  font-size: 0.86rem;
  line-height: 1.7;
  color: var(--va-c-text-2, #98989f);
  margin-bottom: 0.8rem;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.post-card-meta {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
  font-size: 0.74rem;
  color: var(--va-c-text-3, #6a6a71);
  font-family: var(--va-font-mono, monospace);
}

.meta-dot {
  opacity: 0.5;
}

.meta-tag {
  padding: 0.1rem 0.45rem;
  border-radius: 4px;
  background: var(--yun-surface-line, rgba(122, 122, 122, 0.12));
  color: var(--va-c-link, #4cc9f0);
}

/* ==================== 响应式（照抄上游断点 768px） ==================== */
@media screen and (max-width: 768px) {
  #yun-banner {
    --banner-char-size: 2rem;
    --banner-empty-border-size: 2rem;
  }

  .banner-row {
    gap: 0.25rem;
  }

  .site-intro {
    padding: 2rem 1rem 1.5rem;
  }

  .yun-posts {
    padding: 0.5rem 1rem 3rem;
  }
}

/* ==================== 动效可访问性 ==================== */
@media (prefers-reduced-motion: reduce) {
  .banner-line.enter {
    animation: none;
    height: var(--banner-line-height);
  }

  .char-left,
  .char-right {
    animation: none;
    opacity: 1;
  }

  .char-left {
    border-right-width: var(--banner-empty-border-size);
  }

  .char-right {
    border-left-width: var(--banner-empty-border-size);
  }

  .go-down {
    animation: none;
  }

  .post-card:hover {
    transform: none;
  }
}
</style>
