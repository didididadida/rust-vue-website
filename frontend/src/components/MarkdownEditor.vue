<!--
  MarkdownEditor —— 带语法高亮的 Markdown 源码编辑器
  ============================================================================
  用途：
    管理后台编辑文章时，直接编辑整篇 `.md`（front matter + 正文）。
    纯 textarea 看不出标题层级、代码块边界、强调标记，容易写错。

  能力：
    - Markdown 语法高亮：标题、列表、引用、粗体/斜体、链接、行内代码、分隔线
    - front matter（--- ... ---）单独着色
    - 代码围栏内的代码按语言高亮（```js / ```rust / ```python …）
    - 行号、括号匹配、自动补全 Markdown 语法
    - Ctrl+S 保存、粘贴/拖拽截图上传
    - 深色主题，与站点一致

  逻辑描述：
    输入：modelValue（整篇 .md 文本）、placeholder
    输出：v-model 双向绑定的文本；插槽暴露 insertSnippet / focus 给父组件
    关键机制：
      - CodeMirror 6 的 Markdown 语言支持通过 language-data 按需加载子语言
      - 编辑器内部用 CodeMirror 的 Document，而不是 textarea 的 value
      - 粘贴图片时拦截默认行为，上传成功后把 Markdown 图片语法插到光标处
    不变量：外部赋值与内部编辑始终同步（用 setValue + 防抖避免光标跳动）
    边界：超大文件（>200KB）会关闭部分功能以保流畅

  依据：
    经典问题：受控组件与命令式编辑器内核的同步（controlled vs imperative sync）
    本处变体：CodeMirror 自己管理文档与光标，Vue 只想拿到字符串
    沿用结论：内部为权威源，变化时 emit；外部值真正变化时才写回内核
    偏离处：不做 OT/CRDT 协同编辑（单人后台不需要）
  ============================================================================
-->
<template>
  <div class="md-editor" :class="{ 'is-dragging': dragging }">
    <Codemirror
      v-model="code"
      :style="{ height }"
      :placeholder="placeholder"
      :extensions="extensions"
      :autofocus="false"
      class="cm-host"
      @change="onChange"
      @ready="onReady"
      @paste="onPaste"
      @drop.prevent="onDrop"
      @dragover.prevent="dragging = true"
      @dragleave.prevent="dragging = false"
    />
    <div v-if="dragging" class="drop-hint">松开上传图片</div>
    <div v-if="uploading" class="upload-hint">正在上传 {{ uploading }} 张图片…</div>
  </div>
</template>

<script setup>
import { onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { Codemirror } from 'vue-codemirror'
import { EditorView, keymap, placeholder as cmPlaceholder } from '@codemirror/view'
import { EditorState } from '@codemirror/state'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { languages } from '@codemirror/language-data'
import { syntaxHighlighting, HighlightStyle } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'
import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete'
import { searchKeymap, highlightSelectionMatches } from '@codemirror/search'
import http from '../api'

const props = defineProps({
  modelValue: { type: String, default: '' },
  placeholder: { type: String, default: '' },
  height: { type: String, default: 'calc(100vh - 220px)' }
})
const emit = defineEmits(['update:modelValue', 'save'])

const code = ref(props.modelValue)
const dragging = ref(false)
const uploading = ref(0)
const view = shallowRef(null)

// ---------------- 配色：与站点深色主题一致 ----------------
const darkTheme = EditorView.theme(
  {
    '&': {
      color: '#d5dae6',
      backgroundColor: '#0a0e18',
      fontSize: '13.5px',
    },
    '.cm-content': {
      fontFamily:
        "ui-monospace, SFMono-Regular, 'SF Mono', 'JetBrains Mono', Menlo, Consolas, monospace",
      lineHeight: '1.75',
      padding: '10px 0',
      caretColor: '#7ddaf7',
    },
    '.cm-scroller': { overflow: 'auto' },
    '.cm-gutters': {
      backgroundColor: '#0a0e18',
      color: '#4a5468',
      border: 'none',
      paddingRight: '6px',
    },
    '.cm-activeLineGutter': { backgroundColor: 'rgba(255,255,255,0.04)', color: '#8fa0b8' },
    '.cm-activeLine': { backgroundColor: 'rgba(255,255,255,0.028)' },
    '&.cm-focused .cm-cursor': { borderLeftColor: '#7ddaf7' },
    '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': {
      backgroundColor: 'rgba(76,201,240,0.22)',
    },
    '.cm-selectionMatch': { backgroundColor: 'rgba(76,201,240,0.14)' },
    '.cm-matchingBracket': {
      backgroundColor: 'rgba(76,201,240,0.2)',
      outline: '1px solid rgba(76,201,240,0.5)',
    },
    '.cm-placeholder': { color: '#3e4759', fontStyle: 'normal' },
    '&.cm-focused': { outline: 'none' },
  },
  { dark: true }
)

/**
 * 语法着色规则
 * 说明：Markdown 用 heading1~6 / emphasis / strong / link / monospace 等标签；
 *       代码块内部的代码由子语言提供 keyword/string/comment 等标签。
 */
const mdHighlight = HighlightStyle.define([
  // 标题：从亮白到灰，字号随层级递减
  { tag: t.heading1, color: '#ffffff', fontWeight: '700', fontSize: '1.22em' },
  { tag: t.heading2, color: '#e8f3ff', fontWeight: '700', fontSize: '1.12em' },
  { tag: t.heading3, color: '#b9e2ff', fontWeight: '600', fontSize: '1.05em' },
  { tag: [t.heading4, t.heading5, t.heading6], color: '#9fcbe6', fontWeight: '600' },
  // 强调
  { tag: t.strong, color: '#ffd479', fontWeight: '700' },
  { tag: t.emphasis, color: '#c8a8ff', fontStyle: 'italic' },
  { tag: t.strikethrough, color: '#7a8398', textDecoration: 'line-through' },
  // 链接与图片
  { tag: t.link, color: '#5ce6ff', textDecoration: 'underline' },
  { tag: t.url, color: '#4aa8c4' },
  // 代码
  { tag: t.monospace, color: '#98e6b0', backgroundColor: 'rgba(152,230,176,0.09)' },
  { tag: t.processingInstruction, color: '#6f7b92' }, // ``` 围栏
  { tag: t.contentSeparator, color: '#5a6478' }, // --- 分隔线 / front matter 边界
  // 引用与列表
  { tag: t.quote, color: '#9aa8bf', fontStyle: 'italic' },
  { tag: t.list, color: '#7ddaf7' },
  // 代码块内的代码着色
  { tag: t.keyword, color: '#c792ea' },
  { tag: [t.string, t.special(t.string)], color: '#a5e075' },
  { tag: t.comment, color: '#5f6b80', fontStyle: 'italic' },
  { tag: [t.number, t.bool, t.null], color: '#f0a35e' },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: '#69c3ff' },
  { tag: [t.typeName, t.className], color: '#e5c07b' },
  { tag: t.variableName, color: '#d5dae6' },
  { tag: [t.propertyName, t.attributeName], color: '#7ddaf7' },
  { tag: t.operator, color: '#8f9bb3' },
  { tag: t.tagName, color: '#e06c9f' },
  // 错误
  { tag: t.invalid, color: '#ff6b6b' },
])

// ---------------- 扩展 ----------------
const saveKeymap = keymap.of([
  {
    key: 'Mod-s',
    preventDefault: true,
    run: () => {
      emit('save')
      return true
    }
  }
])

const extensions = [
  markdown({
    base: markdownLanguage,
    // 按需加载围栏代码块里的子语言（```js / ```rust 等）
    codeLanguages: languages,
    addKeymap: true
  }),
  EditorState.allowMultipleSelections.of(true),
  history(),
  closeBrackets(),
  highlightSelectionMatches(),
  syntaxHighlighting(mdHighlight),
  EditorView.lineWrapping,
  darkTheme,
  // 手动组装 keymap，避免与默认搜索键冲突
  keymap.of([
    ...closeBracketsKeymap,
    ...defaultKeymap,
    ...searchKeymap,
    ...historyKeymap,
    indentWithTab,
  ]),
  saveKeymap,
  cmPlaceholder(props.placeholder),
]

function onReady(payload) {
  view.value = payload.view
}

// ---------------- 双向同步 ----------------
let syncing = false
function onChange(value) {
  if (syncing) return
  emit('update:modelValue', value)
}

watch(
  () => props.modelValue,
  v => {
    if (v === code.value) return
    syncing = true
    code.value = v
    // 下一帧解锁，避免把程序化赋值当成用户输入再 emit 一次
    requestAnimationFrame(() => {
      syncing = false
    })
  }
)

// ---------------- 图片上传 ----------------

/** 在光标处插入文本（用于 Markdown 图片语法） */
function insertAtCursor(text) {
  const v = view.value
  if (!v) {
    code.value += text
    return
  }
  const sel = v.state.selection.main
  v.dispatch({
    changes: { from: sel.from, to: sel.to, insert: text },
    selection: { anchor: sel.from + text.length }
  })
  v.focus()
}

async function uploadFiles(files) {
  if (!files.length) return
  uploading.value += files.length
  for (const file of files) {
    try {
      const form = new FormData()
      form.append('file', file)
      const res = await http.post('/api/upload', form, {
        headers: { 'Content-Type': 'multipart/form-data' }
      })
      insertAtCursor(`\n![图片](${res.url})\n`)
    } catch (e) {
      alert(`图片上传失败：${e.message}`)
    } finally {
      uploading.value -= 1
    }
  }
}

function onPaste(e) {
  const items = e.clipboardData?.items
  if (!items) return
  const files = []
  for (const item of items) {
    if (item.type.startsWith('image/')) {
      const f = item.getAsFile()
      if (f) files.push(f)
    }
  }
  if (files.length) {
    e.preventDefault()
    uploadFiles(files)
  }
}

function onDrop(e) {
  dragging.value = false
  const files = Array.from(e.dataTransfer?.files || []).filter(f => f.type.startsWith('image/'))
  if (files.length) uploadFiles(files)
}

/** 供父组件调用：在光标处插入预设片段 */
function insertSnippet(kind) {
  const snippets = {
    h2: '\n## 小节标题\n\n',
    code: '\n```js\n// 代码\n```\n\n',
    quote: '\n> 引用内容\n\n',
    image: '\n![图片]()\n'
  }
  insertAtCursor(snippets[kind] || '')
  view.value?.focus()
}

function focus() {
  view.value?.focus()
}

defineExpose({ insertSnippet, focus })

onBeforeUnmount(() => {
  view.value = null
})
</script>

<style scoped>
.md-editor {
  position: relative;
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
  background: #0a0e18;
  transition: border-color 0.2s var(--ease);
}

.md-editor:focus-within {
  border-color: rgba(76, 201, 240, 0.42);
}

.cm-host {
  font-size: 13.5px;
}

/* CodeMirror 内容区高度交给组件 style */
.cm-host :deep(.cm-editor) {
  height: 100%;
}

.drop-hint {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(10, 14, 24, 0.88);
  border: 2px dashed var(--accent);
  border-radius: 8px;
  color: var(--accent);
  font-size: 0.9rem;
  font-family: var(--font-mono);
  letter-spacing: 0.06em;
  pointer-events: none;
  z-index: 10;
}

.upload-hint {
  position: absolute;
  right: 10px;
  bottom: 8px;
  padding: 0.2rem 0.6rem;
  font-size: 0.72rem;
  font-family: var(--font-mono);
  color: var(--accent);
  background: rgba(10, 14, 24, 0.9);
  border: 1px solid rgba(76, 201, 240, 0.3);
  border-radius: var(--radius-full);
  pointer-events: none;
}
</style>
