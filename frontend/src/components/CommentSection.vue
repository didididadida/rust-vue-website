<template>
  <section class="comments">
    <h2 class="comments-title">
      评论
      <span v-if="loaded" class="comments-count">{{ total }}</span>
    </h2>

    <p v-if="loading" class="comments-hint">评论加载中...</p>
    <p v-else-if="total === 0" class="comments-hint">还没有评论，来说两句吧～</p>

    <!-- 评论列表 -->
    <ul v-else class="comment-list">
      <li v-for="item in flatList" :key="item.id" class="comment-item" :class="{ 'is-reply': item.depth > 0 }">
        <div class="comment-avatar">{{ initial(item.author) }}</div>
        <div class="comment-main">
          <div class="comment-head">
            <span class="comment-author">{{ item.author }}</span>
            <time class="comment-time">{{ formatDateTime(item.created_at) }}</time>
            <span v-if="!item.is_approved" class="comment-pending">待审核</span>
          </div>
          <p class="comment-body">{{ item.content }}</p>
          <div class="comment-actions">
            <button type="button" class="link-btn" @click="startReply(item)">回复</button>
            <button v-if="auth.isAdmin" type="button" class="link-btn danger" @click="removeComment(item)">删除</button>
          </div>
        </div>
      </li>
    </ul>

    <!-- 发表评论 -->
    <form class="comment-form" @submit.prevent="submit">
      <h3 class="form-title">
        {{ replyTo ? `回复 ${replyTo.author}` : '发表评论' }}
        <button v-if="replyTo" type="button" class="link-btn" @click="cancelReply">取消回复</button>
      </h3>

      <div class="form-row">
        <input
          v-model.trim="form.author"
          class="form-input"
          placeholder="昵称 *"
          maxlength="30"
          required
        />
        <input
          v-model.trim="form.email"
          class="form-input"
          type="email"
          placeholder="邮箱（选填，不会公开）"
          maxlength="120"
        />
      </div>

      <textarea
        ref="textarea"
        v-model.trim="form.content"
        class="form-textarea"
        rows="4"
        placeholder="说点什么…（最多 2000 字）"
        maxlength="2000"
        required
      ></textarea>

      <!-- 评论区工具栏：B 站评论框同款的贴图面板 -->
      <div class="comment-toolbar">
        <span class="emoji-anchor">
          <button
            type="button"
            class="tool-btn"
            :class="{ active: emojiOpen }"
            title="表情"
            @click="emojiOpen = !emojiOpen"
          >
            <span class="tool-svg" v-html="iconSvg('smile', 17)"></span>
          </button>
        </span>
        <span v-if="replyTo" class="replying-hint">正在回复 {{ replyTo.author }}</span>
        <span class="toolbar-spacer"></span>
        <span class="char-count">{{ form.content.length }}/2000</span>
      </div>

      <!-- 蜜罐：正常用户看不到，机器人会填 -->
      <input v-model="form.website" class="honeypot" type="text" tabindex="-1" autocomplete="off" aria-hidden="true" />

      <div class="form-footer">
        <span v-if="notice" :class="['form-notice', noticeType]">{{ notice }}</span>
        <button class="btn" type="submit" :disabled="submitting">
          {{ submitting ? '提交中...' : '发表评论' }}
        </button>
      </div>
    </form>
  </section>
</template>

<script setup>
import { ref, computed, nextTick, onMounted, watch } from 'vue'
import http from '../api'
import { auth } from '../auth'
import { formatDateTime } from '../labels'
import { iconSvg, emojiChar } from '../emoji'

const props = defineProps({
  slug: { type: String, required: true },
})

const items = ref([])
const total = ref(0)
const loading = ref(true)
const loaded = ref(false)
const submitting = ref(false)
const notice = ref('')
const noticeType = ref('success')
const replyTo = ref(null)
const emojiOpen = ref(false)
const textarea = ref(null)

const form = ref({ author: '', email: '', content: '', website: '' })

/**
 * 插入贴图。
 * 评论区是纯文本存储（后端按纯文本保存、前端按 pre-wrap 渲染），
 * 所以这里插入的是贴图对应的字符表示，而不是 SVG 标记。
 *
 * 依据：
 *   经典问题：存储格式与展示能力的错配
 *   本处变体：富文本编辑器可以存 `:key:` 标记再渲染成 SVG，但评论是纯文本
 *   沿用结论：选择降级而不是改造存储 —— 纯文本评论的可审核性、可搜索性都更好
 *   偏离处：视觉上不如 SVG 精致，但在"管理员要能读到内容"的前提下是正确取舍
 */
function insertEmoji({ key }) {
  // 插入 B 站写法 `[key]`，渲染时替换成表情图
  const text = `[${key}]`
  const el = textarea.value
  if (!el) {
    form.value.content += text
    return
  }
  const start = el.selectionStart ?? form.value.content.length
  const end = el.selectionEnd ?? start
  const value = form.value.content
  form.value.content = value.slice(0, start) + text + value.slice(end)
  nextTick(() => {
    el.focus()
    const pos = start + text.length
    el.setSelectionRange(pos, pos)
  })
  emojiOpen.value = false
}

const initial = name => (name || '?').trim().charAt(0).toUpperCase()

/** 把平铺的评论按父子关系展开成带缩进的列表（只支持一层回复） */
const flatList = computed(() => {
  const byId = new Map(items.value.map(item => [item.id, item]))
  const roots = []
  const children = new Map()

  for (const item of items.value) {
    if (item.parent_id && byId.has(item.parent_id)) {
      if (!children.has(item.parent_id)) children.set(item.parent_id, [])
      children.get(item.parent_id).push(item)
    } else {
      roots.push(item)
    }
  }

  const out = []
  for (const root of roots) {
    out.push({ ...root, depth: 0 })
    for (const child of children.get(root.id) || []) {
      out.push({ ...child, depth: 1 })
    }
  }
  return out
})

async function load() {
  loading.value = true
  try {
    const data = await http.get(`/api/articles/${encodeURIComponent(props.slug)}/comments`)
    items.value = data.items || []
    total.value = data.total ?? items.value.length
  } catch (e) {
    console.error('加载评论失败:', e)
    items.value = []
    total.value = 0
  } finally {
    loading.value = false
    loaded.value = true
  }
}

function startReply(item) {
  replyTo.value = item
  notice.value = ''
}

function cancelReply() {
  replyTo.value = null
}

async function submit() {
  notice.value = ''
  submitting.value = true
  try {
    const payload = {
      author: form.value.author,
      content: form.value.content,
      email: form.value.email,
      parent_id: replyTo.value?.id ?? null,
      website: form.value.website,
    }
    const res = await http.post(
      `/api/articles/${encodeURIComponent(props.slug)}/comments`,
      payload
    )

    form.value.content = ''
    form.value.website = ''
    replyTo.value = null

    if (res.requires_review) {
      notice.value = '评论已提交，等待管理员审核后显示。'
      noticeType.value = 'warning'
    } else {
      notice.value = '评论发表成功！'
      noticeType.value = 'success'
      await load()
    }
  } catch (e) {
    notice.value = e.message || '提交失败，请稍后再试'
    noticeType.value = 'error'
  } finally {
    submitting.value = false
  }
}

async function removeComment(item) {
  if (!confirm(`确定删除 ${item.author} 的这条评论？`)) return
  try {
    await http.delete(`/api/comments/${encodeURIComponent(props.slug)}/${item.id}`)
    await load()
  } catch (e) {
    alert('删除失败：' + e.message)
  }
}

onMounted(load)
watch(() => props.slug, () => {
  items.value = []
  replyTo.value = null
  notice.value = ''
  load()
})
</script>

<style scoped>
.comments {
  margin-top: 3.5rem;
  padding-top: 2.5rem;
  border-top: 1px solid var(--border);
}

.comments-title {
  font-family: var(--font-serif);
  font-size: 1.3rem;
  font-weight: 600;
  letter-spacing: 0.03em;
  margin-bottom: 1.75rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.comments-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.6rem;
  height: 1.35rem;
  padding: 0 0.45rem;
  font-family: var(--font-sans);
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--accent-soft);
  background: var(--accent-dim);
  border-radius: var(--radius-full);
}

.comments-hint {
  color: var(--text-muted);
  font-size: 0.9rem;
  padding: 1.25rem 0 2rem;
}

/* ---------- 评论列表 ---------- */
.comment-list {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  margin-bottom: 2.5rem;
}

.comment-item {
  display: flex;
  gap: 0.85rem;
  padding: 1rem;
  border-radius: var(--radius-md);
  transition: background 0.2s var(--ease);
}

.comment-item:hover {
  background: rgba(255, 255, 255, 0.015);
}

/* 回复：左侧缩进 + 细分隔线，弱化层级感过度 */
.comment-item.is-reply {
  margin-left: 2.5rem;
  border-left: 2px solid var(--border);
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
}

.comment-avatar {
  flex-shrink: 0;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-serif);
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--accent-soft);
  background: var(--accent-dim);
  border: 1px solid rgba(123, 108, 246, 0.18);
  user-select: none;
}

.comment-main {
  flex: 1;
  min-width: 0;
}

.comment-head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
  margin-bottom: 0.3rem;
}

.comment-author {
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-primary);
}

.comment-time {
  font-size: 0.75rem;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.comment-pending {
  font-size: 0.7rem;
  color: var(--warning);
  border: 1px solid rgba(217, 178, 107, 0.35);
  background: rgba(217, 178, 107, 0.1);
  padding: 0.05rem 0.4rem;
  border-radius: var(--radius-full);
}

.comment-body {
  font-size: 0.92rem;
  line-height: 1.75;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}

.comment-actions {
  display: flex;
  gap: 0.9rem;
  margin-top: 0.4rem;
}

/* ---------- 文字按钮 ---------- */
.link-btn {
  background: none;
  border: none;
  padding: 0;
  font-size: 0.78rem;
  font-family: inherit;
  color: var(--text-muted);
  cursor: pointer;
  transition: color 0.2s var(--ease);
}

.link-btn:hover {
  color: var(--accent-soft);
}

.link-btn.danger:hover {
  color: var(--danger);
}

/* ---------- 表单 ---------- */
.comment-form {
  background: var(--bg-raised);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: 1.5rem;
}

.form-title {
  font-family: var(--font-sans);
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 1.1rem;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}

.form-input,
.form-textarea {
  width: 100%;
  background: var(--bg-base);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 0.65rem 0.85rem;
  color: var(--text-primary);
  font-size: 0.9rem;
  font-family: inherit;
  line-height: 1.6;
  transition: border-color 0.2s var(--ease), background 0.2s var(--ease);
}

.form-textarea {
  resize: vertical;
  min-height: 6rem;
}

.form-input::placeholder,
.form-textarea::placeholder {
  color: var(--text-muted);
}

.form-input:focus,
.form-textarea:focus {
  outline: none;
  border-color: var(--accent);
  background: var(--bg-overlay);
}

.form-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  margin-top: 1rem;
  flex-wrap: wrap;
}

.form-notice {
  font-size: 0.82rem;
}

.form-notice.success {
  color: var(--success);
}

.form-notice.error {
  color: var(--danger);
}

/* 蜜罐：移出视口而非 display:none —— 许多爬虫会跳过 display:none 的字段 */
.honeypot {
  position: absolute;
  left: -9999px;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}

/* ---------- 评论工具栏（B 站评论框底部那条） ---------- */
.comment-toolbar {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  margin-top: 0.6rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--border);
}

.emoji-anchor {
  position: relative;
  display: inline-flex;
}

.tool-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.95rem;
  height: 1.95rem;
  padding: 0;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.16s var(--ease);
}

.tool-btn:hover,
.tool-btn.active {
  color: var(--accent);
  background: rgba(76, 201, 240, 0.12);
  border-color: rgba(76, 201, 240, 0.28);
}

.tool-svg {
  display: inline-flex;
}

.tool-svg :deep(svg) {
  display: block;
}

.replying-hint {
  font-size: 0.76rem;
  color: var(--text-muted);
}

.toolbar-spacer {
  flex: 1;
}

.char-count {
  font-size: 0.72rem;
  color: var(--text-muted);
  font-family: var(--font-mono);
}

@media (max-width: 640px) {
  .form-row {
    grid-template-columns: 1fr;
  }

  .comment-item.is-reply {
    margin-left: 1rem;
  }

  .comment-form {
    padding: 1.15rem;
  }
}
</style>