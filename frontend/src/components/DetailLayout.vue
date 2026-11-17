<!--
  DetailLayout —— 文章 / 项目 / 工具的公共详情骨架

  逻辑描述：
    输入：kind（article|toy|tool 的显示配置）、标题、元信息插槽、正文 Markdown、
          底部动作插槽、可选的评论 slug
    输出：带右侧分级目录的正文页；正文走统一的 renderMarkdown 渲染
    关键机制：
      - 正文用 DOMParser 解析后回写，给 h1~h3 补 id 作为锚点，同时提取目录
      - 目录高亮用 scroll 事件 + rAF 节流，按"最后一个越过 90px 线的标题"判定当前章节
      - 代码块「一键复制」通过 enableCodeCopy 事件委派挂到 document
    不变量：同一次渲染里每个标题的锚点 id 唯一；scroll / click 监听一挂一解、不成对叠加
    边界与失效：
      - 正文不含 h1~h3 时不渲染目录，布局退回单列
      - 组件卸载必须解绑 scroll 和 click 监听，否则多次进入会叠加

  依据：
    经典问题：哈希索引 vs 顺序扫描（找"当前章节"）+ 幂等绑定（事件监听不重复挂载）
    本处变体：章节 id 由标题文本派生，重名要加数字后缀；章节数通常 < 30，线性扫描足够
    沿用结论：线性扫描 O(n)、每个 scroll 帧最多一次，复杂度与目录长度同阶
    偏离处：标题文本完全相同会退化为 makeHeadingId 的后缀序号，编号随顺序变化，
             但同屏内唯一，足以支撑锚点跳转
-->
<template>
  <div class="detail">
    <div v-if="loading" class="loading">加载中...</div>
    <div v-else-if="!item" class="empty">
      <p>{{ emptyText }}</p>
      <router-link :to="backTo" class="btn" style="margin-top: 16px;">{{ backText }}</router-link>
    </div>
    <div v-else class="detail-layout" :class="{ 'has-toc': toc.length }">
      <!-- 右侧分级目录 -->
      <aside v-if="toc.length" class="toc">
        <p class="toc-title">目录</p>
        <nav class="toc-nav">
          <a
            v-for="entry in toc"
            :key="entry.id"
            class="toc-link"
            :class="{ 'is-active': activeId === entry.id }"
            :style="{ paddingLeft: 10 + entry.indent * 14 + 'px' }"
            :href="'#' + entry.id"
            @click.prevent="scrollToHeading(entry.id)"
          >{{ entry.text }}</a>
        </nav>
      </aside>

      <article class="detail-content">
        <!-- 主视觉：封面 / 图标 -->
        <div v-if="item.cover_image" class="detail-cover">
          <img :src="item.cover_image" :alt="item.title" loading="lazy" />
        </div>
        <div v-else-if="item.icon" class="detail-icon">{{ item.icon }}</div>

        <header class="detail-header">
          <div class="detail-meta">
            <span class="category-badge">{{ item.badge }}</span>
            <template v-if="item.date">
              <span class="dot">·</span>
              <span class="date">{{ item.date }}</span>
            </template>
          </div>
          <h1 class="detail-title">{{ item.title }}</h1>
          <p v-if="item.summary" class="detail-summary">{{ item.summary }}</p>
          <div v-if="item.tags && item.tags.length" class="detail-tags">
            <span v-for="tag in item.tags" :key="tag" class="tag">{{ tag }}</span>
          </div>
        </header>

        <!-- 正文 -->
        <!-- 正文：v-html 渲染后，挂载其中的视频占位（video.js 需要真实 DOM 才能初始化） -->
        <div v-if="hasBody" ref="bodyEl" class="detail-body markdown-body" v-html="html"></div>
        <p v-else class="detail-empty-body">还没有正文内容。</p>

        <!-- 底部动作（在线体验 / 源代码 / 打开工具 ...），由 normalize 产出 -->
        <div v-if="item.actions && item.actions.length" class="detail-actions">
          <a
            v-for="action in item.actions"
            :key="action.href"
            :class="['btn', { 'btn-outline': action.ghost }]"
            :href="action.href"
            target="_blank"
            rel="noopener"
          >{{ action.label }}</a>
        </div>

        <!-- 评论 -->
        <CommentSection v-if="commentsSlug" :slug="commentsSlug" />

        <div class="detail-footer">
          <router-link :to="backTo" class="btn btn-outline">{{ backText }}</router-link>
        </div>
      </article>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import http from '../api'
import { renderMarkdown, enableCodeCopy, sanitizeHtmlSync } from '../markdown'
import CommentSection from './CommentSection.vue'

const props = defineProps({
  /** 接口路径前缀，例如 /api/articles、/api/toys、/api/tools */
  apiBase: { type: String, required: true },
  /** 把接口返回的对象转成统一的展示结构 */
  normalize: { type: Function, required: true },
  backTo: { type: String, default: '/' },
  backText: { type: String, default: '返回列表' },
  emptyText: { type: String, default: '内容不存在或已被删除' },
  /** 传了才渲染评论区 */
  commentsSlug: { type: String, default: '' }
})

const route = useRoute()
const raw = ref(null)
const loading = ref(true)
const activeId = ref('')
const bodyEl = ref(null)

let detachCopy = null
let ticking = false

const item = computed(() => (raw.value ? props.normalize(raw.value) : null))
const hasBody = computed(() => !!(raw.value && String(raw.value.content || '').trim()))

/** 锚点 id：去掉特殊字符，重名按出现顺序加数字后缀（同一次渲染内唯一即可） */
function makeHeadingId(text, used) {
  const base =
    String(text)
      .trim()
      .toLowerCase()
      .replace(/\s+/g, '-')
      .replace(/[^\p{L}\p{N}\-_]/gu, '') || 'section'
  let id = base
  let n = 2
  while (used.has(id)) id = `${base}-${n++}`
  used.add(id)
  return id
}

/** 渲染正文：补标题 id + 提取目录 */
const rendered = computed(() => {
  const md = raw.value?.content
  if (!String(md || '').trim()) return { html: '', toc: [] }

  // 净化后再交给 v-html（见 markdown.js 的 sanitizeHtmlSync）
  const doc = new DOMParser().parseFromString(sanitizeHtmlSync(renderMarkdown(md)), 'text/html')
  const headings = [...doc.querySelectorAll('h1, h2, h3')]
  const used = new Set()

  // 目录层级按"当前出现的最小标题级别"归一，避免正文从 h2 开始时全部缩进
  const minLevel = headings.length ? Math.min(...headings.map(h => Number(h.tagName[1]))) : 1

  const toc = headings.map(h => {
    const level = Number(h.tagName[1])
    const id = makeHeadingId(h.textContent, used)
    h.id = id
    return { id, text: h.textContent.trim(), level, indent: level - minLevel }
  })

  return { html: doc.body.innerHTML, toc }
})

const html = computed(() => rendered.value.html)
const toc = computed(() => rendered.value.toc)

function scrollToHeading(id) {
  const el = document.getElementById(id)
  if (!el) return
  window.scrollTo({ top: el.getBoundingClientRect().top + window.scrollY - 84, behavior: 'smooth' })
  activeId.value = id
}

/** 当前章节 = 最后一个顶边越过 90px 参考线的标题 */
function updateActive() {
  const entries = toc.value
  if (!entries.length) return
  let current = entries[0].id
  for (const entry of entries) {
    const el = document.getElementById(entry.id)
    if (el && el.getBoundingClientRect().top <= 90) current = entry.id
    else break
  }
  if (window.innerHeight + window.scrollY >= document.body.scrollHeight - 2) {
    current = entries[entries.length - 1].id
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

async function load() {
  loading.value = true
  const slug = route.params.slug
  try {
    raw.value = await http.get(`${props.apiBase}/${encodeURIComponent(slug)}`)
  } catch (e) {
    console.error(`加载 ${props.apiBase} 失败:`, e)
    raw.value = null
  } finally {
    loading.value = false
  }
  await nextTick()
  updateActive()
  // 正文换了（目录跟着变），复制按钮的 id 也换，重新挂一次事件委派即可
  detachCopy?.()
  detachCopy = enableCodeCopy()
  // 视频占位在本轮 DOM 更新后挂载成 video.js 播放器
}

onMounted(() => {
  load()
  window.addEventListener('scroll', onScroll, { passive: true })
})

// 同一组件在不同 slug 间跳转时（例如从项目跳到工具），复用实例重新加载
watch(
  () => route.params.slug,
  slug => {
    if (slug) load()
  }
)

onBeforeUnmount(() => {
  window.removeEventListener('scroll', onScroll)
  detachCopy?.()
})
</script>

<style scoped>
.detail-layout.has-toc {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 220px;
  gap: 3.5rem;
  align-items: start;
}

.detail-content {
  order: 1;
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

/* 没有封面时用一个大号 Emoji 当主视觉 */
.detail-icon {
  font-size: 3.25rem;
  line-height: 1;
  margin-bottom: 1.5rem;
  filter: drop-shadow(0 0 18px rgba(76, 201, 240, 0.35));
}

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

.detail-title {
  font-family: var(--font-serif);
  font-size: clamp(1.75rem, 4.5vw, 2.4rem);
  font-weight: 600;
  line-height: 1.35;
  letter-spacing: 0.01em;
  margin-bottom: 1.1rem;
  color: var(--text-primary);
}

.detail-summary {
  font-size: 0.98rem;
  color: var(--text-secondary);
  line-height: 1.85;
  margin-bottom: 1.1rem;
}

.detail-tags {
  display: flex;
  flex-wrap: wrap;
}

.detail-body {
  font-size: 1.0625rem;
  line-height: 1.9;
}

.detail-empty-body {
  color: var(--text-muted);
  font-size: 0.92rem;
}

.detail-actions {
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
  margin-top: 2.25rem;
}

.detail-footer {
  margin-top: 3rem;
  padding-top: 1.75rem;
  border-top: 1px solid var(--border);
}

@media (max-width: 1080px) {
  .detail-layout.has-toc {
    grid-template-columns: minmax(0, 1fr);
    gap: 0;
  }

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
