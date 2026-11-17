<template>
  <div class="blog-detail">
    <div v-if="loading" class="loading">加载中...</div>
    <div v-else-if="!article" class="empty">
      <p>文章不存在或已被删除</p>
      <router-link to="/blog" class="btn" style="margin-top: 16px;">返回博客列表</router-link>
    </div>
    <div v-else class="detail-layout" :class="{ 'has-toc': rendered.toc.length }">
      <!-- 左侧分级目录 -->
      <aside v-if="rendered.toc.length" class="toc">
        <p class="toc-title">目录</p>
        <nav class="toc-nav">
          <a
            v-for="item in rendered.toc"
            :key="item.id"
            class="toc-link"
            :class="{ 'is-active': activeId === item.id }"
            :style="{ paddingLeft: 10 + item.indent * 14 + 'px' }"
            :href="'#' + item.id"
            @click.prevent="scrollToHeading(item.id)"
          >{{ item.text }}</a>
        </nav>
      </aside>

      <article class="article-content">
        <!-- 封面图 -->
        <div v-if="article.cover_image" class="detail-cover">
          <img :src="article.cover_image" :alt="article.title" loading="lazy" />
        </div>

        <!-- 文章头部 -->
        <header class="detail-header">
          <div class="detail-meta">
            <span class="category-badge">{{ categoryLabel(article.category) }}</span>
            <span class="dot">·</span>
            <span class="date">{{ formatDate(article.created_at) }}</span>
            <span v-if="article.updated_at !== article.created_at" class="date updated">
              · 更新于 {{ formatDate(article.updated_at) }}
            </span>
          </div>
          <h1 class="detail-title">{{ article.title }}</h1>
          <div v-if="article.tags && article.tags.length" class="detail-tags">
            <span v-for="tag in article.tags" :key="tag" class="tag">{{ tag }}</span>
          </div>
        </header>

        <!-- 正文 -->
        <div ref="bodyRef" class="detail-body markdown-body" v-html="rendered.html"></div>

        <!-- 评论 -->
        <CommentSection :slug="article.slug" />

        <!-- 底部导航 -->
        <div class="detail-footer">
          <router-link to="/blog" class="btn btn-outline">返回文章列表</router-link>
        </div>
      </article>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import http from '../api'
import { categoryLabel, formatDate } from '../labels'
import { renderMarkdown, enableCodeCopy, sanitizeHtmlSync } from '../markdown'
import CommentSection from '../components/CommentSection.vue'

const route = useRoute()
const article = ref(null)
const loading = ref(true)
const activeId = ref('')
const bodyRef = ref(null)

let detachCopy = null // 代码块「一键复制」的解绑函数

// 锚点 id：去掉特殊字符，重名的加数字后缀
const usedIds = new Set()
function makeHeadingId(text) {
  const base =
    String(text)
      .trim()
      .toLowerCase()
      .replace(/\s+/g, '-')
      .replace(/[^\p{L}\p{N}\-_]/gu, '') || 'section'
  let id = base
  let n = 2
  while (usedIds.has(id)) id = `${base}-${n++}`
  usedIds.add(id)
  return id
}

// 渲染正文：给 h1~h3 补上 id，并顺便提取分级目录
const rendered = computed(() => {
  const raw = article.value?.content
  if (!raw) return { html: '', toc: [] }

  // 先净化再解析：DOMParser 解析文档不会执行脚本，但 innerHTML 交给 v-html 会，
  // 所以净化必须在「产出交给 v-html 的那份 HTML」上做（见 markdown.js 的 sanitizeHtml）
  const doc = new DOMParser().parseFromString(sanitizeHtmlSync(renderMarkdown(raw)), 'text/html')
  const headings = [...doc.querySelectorAll('h1, h2, h3')]
  usedIds.clear()

  const minLevel = headings.length
    ? Math.min(...headings.map((h) => Number(h.tagName[1])))
    : 1

  const toc = headings.map((h) => {
    const level = Number(h.tagName[1])
    const id = makeHeadingId(h.textContent)
    h.id = id
    return { id, text: h.textContent.trim(), level, indent: level - minLevel }
  })

  return { html: doc.body.innerHTML, toc }
})

function scrollToHeading(id) {
  const el = document.getElementById(id)
  if (!el) return
  window.scrollTo({ top: el.getBoundingClientRect().top + window.scrollY - 84, behavior: 'smooth' })
  activeId.value = id
}

// 滚动时高亮当前所在的章节
let ticking = false
function updateActive() {
  const items = rendered.value.toc
  if (!items.length) return
  let current = items[0].id
  for (const item of items) {
    const el = document.getElementById(item.id)
    if (el && el.getBoundingClientRect().top <= 90) current = item.id
    else break
  }
  if (window.innerHeight + window.scrollY >= document.body.scrollHeight - 2) {
    current = items[items.length - 1].id
  }
  activeId.value = current
}

function onScroll() {
  if (ticking) return
  ticking = true
  requestAnimationFrame(() => {
    updateActive()
    ticking = false
  })
}

onMounted(async () => {
  const slug = route.params.slug
  try {
    article.value = await http.get(`/api/articles/${encodeURIComponent(slug)}`)
  } catch (e) {
    console.error('加载文章失败:', e)
    article.value = null
  } finally {
    loading.value = false
  }
  await nextTick()
  updateActive()
  window.addEventListener('scroll', onScroll, { passive: true })
  detachCopy = enableCodeCopy()
  // 正文里的视频占位挂载成 video.js 播放器
})

onBeforeUnmount(() => {
  window.removeEventListener('scroll', onScroll)
  detachCopy?.()
})
</script>

<style scoped>
/* ---------- 布局：有目录时右侧预留 220px ---------- */
.detail-layout.has-toc {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 220px;
  gap: 3.5rem;
  align-items: start;
}

/* 目录放右侧，正文在左，符合"边读边看定位"的习惯 */
.article-content {
  order: 1;
  /* 限制正文宽度，中文长文超过 40 字/行就难读了 */
  max-width: 100%;
}

.toc {
  order: 2;
  position: sticky;
  top: 88px;
  max-height: calc(100vh - 120px);
  overflow-y: auto;
  padding: 0.25rem 0 0 1.25rem;
  border-left: 1px solid var(--border);
  /* 细滚动条，避免目录区出现粗重滚动条 */
  scrollbar-width: thin;
}

.toc::-webkit-scrollbar {
  width: 4px;
}

.toc-title {
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  color: var(--text-muted);
  margin-bottom: 0.85rem;
}

.toc-nav {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
}

.toc-link {
  display: block;
  font-size: 0.82rem;
  line-height: 1.55;
  color: var(--text-muted);
  padding: 0.3rem 0.6rem;
  border-radius: var(--radius-sm);
  word-break: break-word;
  transition: color 0.2s var(--ease), background 0.2s var(--ease);
}

.toc-link:hover {
  color: var(--text-primary);
  background: var(--bg-overlay);
}

.toc-link.is-active {
  color: var(--accent-soft);
  background: var(--accent-dim);
  font-weight: 500;
}

/* ---------- 封面 ---------- */
.detail-cover {
  width: 100%;
  max-height: 380px;
  overflow: hidden;
  border-radius: var(--radius-lg);
  margin-bottom: 2.25rem;
  background: var(--bg-overlay);
}

.detail-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/* ---------- 文章头部 ---------- */
.detail-header {
  margin-bottom: 2.5rem;
  padding-bottom: 1.75rem;
  border-bottom: 1px solid var(--border);
}

.detail-meta {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  margin-bottom: 1rem;
  flex-wrap: wrap;
}

.category-badge {
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  color: var(--accent);
  background: var(--accent-dim);
  padding: 0.15rem 0.55rem;
  border-radius: var(--radius-full);
  border: 1px solid rgba(123, 108, 246, 0.2);
}

.dot {
  color: var(--text-muted);
  font-size: 0.75rem;
}

.date {
  font-size: 0.8rem;
  color: var(--text-muted);
  letter-spacing: 0.02em;
}

.date.updated {
  font-style: normal;
  opacity: 0.8;
}

.detail-title {
  font-family: var(--font-serif);
  font-size: clamp(1.75rem, 4.5vw, 2.4rem);
  font-weight: 600;
  line-height: 1.35;
  letter-spacing: 0.01em;
  margin-bottom: 1.1rem;
  color: var(--text-primary);
}

.detail-tags {
  display: flex;
  flex-wrap: wrap;
}

/* ---------- 正文 ---------- */
.detail-body {
  font-size: 1.0625rem;
  line-height: 1.9;
}

/* 正文标题留出粘性页头高度，锚点跳转不被遮挡 */
.detail-body :deep(h1),
.detail-body :deep(h2),
.detail-body :deep(h3) {
  scroll-margin-top: 88px;
}

/* ---------- 底部 ---------- */
.detail-footer {
  margin-top: 3rem;
  padding-top: 1.75rem;
  border-top: 1px solid var(--border);
}

/* ---------- 响应式 ---------- */
@media (max-width: 1080px) {
  .detail-layout.has-toc {
    grid-template-columns: minmax(0, 1fr);
    gap: 0;
  }

  /* 窄屏隐藏目录，正文占满 */
  .toc {
    display: none;
  }
}

@media (max-width: 768px) {
  .detail-title {
    font-size: 1.6rem;
    line-height: 1.4;
  }

  .detail-header {
    margin-bottom: 2rem;
    padding-bottom: 1.35rem;
  }

  .detail-cover {
    max-height: 220px;
    margin-bottom: 1.75rem;
  }
}
</style>