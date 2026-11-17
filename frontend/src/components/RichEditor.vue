<template>
  <!--
    B 站评论框式编辑器
    ------------------------------------------------------------------
    收起态：一行输入框，点击展开
    展开态：大输入区 + 底部工具栏（B 站风格）
    - 内部用 contenteditable 做富文本，提交时转成 Markdown
    - 支持粘贴截图 / 拖拽图片自动上传
    - 用户完全不需要懂 Markdown
  -->
  <div class="editor">
    <!-- 收起态：占位提示 -->
    <div v-if="!expanded" class="collapsed" @click="expand">
      <div class="fake-input">{{ placeholder }}</div>
    </div>

    <!-- 展开态 -->
    <div v-else class="expanded-body">
      <!-- 标题（仅文章/帖子需要） -->
      <input
        v-if="showTitle"
        ref="titleEl"
        :value="title"
        class="title-input"
        type="text"
        :placeholder="titlePlaceholder"
        maxlength="120"
        @input="onTitleInput"
      />

      <!-- 正文编辑区 -->
      <div class="body-wrap">
        <div
          ref="bodyEl"
          class="body-input"
          contenteditable="true"
          :data-placeholder="bodyPlaceholder"
          @input="onInput"
          @paste="onPaste"
          @dragover.prevent="dragging = true"
          @dragleave.prevent="dragging = false"
          @drop.prevent="onDrop"
          @keydown.ctrl.b.prevent="exec('bold')"
          @keydown.ctrl.i.prevent="exec('italic')"
          @keydown.ctrl.u.prevent="exec('underline')"
          @blur="onBodyBlur"
        ></div>

        <div v-if="dragging" class="drop-overlay">
          <span>松开上传图片</span>
        </div>
      </div>

      <div v-if="uploading > 0" class="upload-hint">
        正在上传 {{ uploading }} 个文件…<template v-if="videoProgress"> {{ videoProgress }}%</template>
      </div>

      <!-- 工具栏（B 站风格） -->
      <div class="toolbar">
        <div class="tools-left">
          <button type="button" class="tool" title="粗体 Ctrl+B" @mousedown.prevent @click="exec('bold')">
            <b>B</b>
          </button>
          <button type="button" class="tool" title="斜体 Ctrl+I" @mousedown.prevent @click="exec('italic')">
            <i>I</i>
          </button>
          <button type="button" class="tool" title="下划线 Ctrl+U" @mousedown.prevent @click="exec('underline')">
            <u>U</u>
          </button>
          <button type="button" class="tool" title="删除线" @mousedown.prevent @click="exec('strikeThrough')">
            <s>S</s>
          </button>

          <span class="tool-sep"></span>

          <button type="button" class="tool" title="无序列表" @mousedown.prevent @click="insertList('ul')">☰</button>
          <button type="button" class="tool" title="有序列表" @mousedown.prevent @click="insertList('ol')">1.</button>

          <span class="tool-sep"></span>

          <!-- 表情面板（B 站评论框同款） -->
          <span class="tool-emoji">
            <button
              type="button"
              class="tool"
              :class="{ active: emojiOpen }"
              title="表情"
              @mousedown.prevent
              @click="emojiOpen = !emojiOpen"
            >
              <span class="tool-svg" v-html="iconSvg('smile', 17)"></span>
            </button>
          </span>

          <template v-if="showImage">
            <span class="tool-sep"></span>
            <button
              type="button"
              class="tool"
              title="插入图片（也可直接粘贴或拖拽）"
              @mousedown.prevent
              @click="pickImage"
            >
              <span class="tool-svg" v-html="iconSvg('image', 17)"></span>
            </button>
            <input
              ref="fileEl"
              type="file"
              accept="image/*"
              multiple
              class="hidden-file"
              @change="onFilePick"
            />
          </template>

          <template v-if="showVideo">
            <button
              type="button"
              class="tool"
              title="插入视频（也可拖拽视频文件）"
              @mousedown.prevent
              @click="pickVideo"
            >
              <span class="tool-svg" v-html="iconSvg('video', 17)"></span>
            </button>
            <input
              ref="videoEl"
              type="file"
              accept="video/*"
              class="hidden-file"
              @change="onVideoPick"
            />
          </template>
        </div>

        <span class="counter" :class="{ over: count > maxLength }">{{ count }}/{{ maxLength }}</span>
      </div>
    </div>

    <!-- 操作栏 -->
    <div v-if="expanded && !hideActions" class="actions">
      <button type="button" class="btn-cancel" @click="collapse">取消</button>
      <button type="button" class="btn-publish" :disabled="!canSubmit || submitting" @click="submit">
        {{ submitting ? '发布中…' : submitText }}
      </button>
    </div>

    <p v-if="error" class="err">{{ error }}</p>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import http from '../api'
import { iconSvg, emojiChar } from '../emoji'

const props = defineProps({
  modelValue: { type: String, default: '' },   // 正文 Markdown
  title: { type: String, default: '' },
  showTitle: { type: Boolean, default: false },
  showImage: { type: Boolean, default: true },
  // 是否提供视频上传入口（社区发帖用）
  showVideo: { type: Boolean, default: false },
  placeholder: { type: String, default: '说点什么…' },
  titlePlaceholder: { type: String, default: '标题' },
  bodyPlaceholder: { type: String, default: '写点什么吧，支持粘贴截图' },
  maxLength: { type: Number, default: 5000 },
  submitText: { type: String, default: '发布' },
  submitting: { type: Boolean, default: false },
  autoFocus: { type: Boolean, default: false },
  // 隐藏内置操作栏（由父组件自己渲染按钮时用）
  hideActions: { type: Boolean, default: false }
})

const emit = defineEmits(['update:modelValue', 'update:title', 'submit'])

const expanded = ref(false)
const bodyEl = ref(null)
const titleEl = ref(null)
const fileEl = ref(null)
const videoEl = ref(null)
const emojiOpen = ref(false)
const videoProgress = ref(0)
let savedRange = null
const dragging = ref(false)
const uploading = ref(0)
const error = ref('')
const title = ref(props.title)

/**
 * 双向绑定同步器（标题 / 正文共用）
 *
 * 输入 → 输出：本地 ref ⇄ 父组件状态，两个方向都闭合
 * 关键机制：子 → 父用 emit，父 → 子用 watch(props)，判等后写入截断回环
 * 不变量：外部每次改写 props，本地 ref 都会跟着变；本地每次编辑，父组件都会拿到新值
 * 边界：只适用于字符串这类值语义类型；对象/数组需按引用比对
 *
 * 依据：
 *   经典问题：受控组件 / 双向绑定（two-way binding）的单数据源原则
 *   本处变体：子组件用本地 ref 暂存编辑值，props 仅作为「外部权威值」的同步源
 *   沿用结论：双向绑定必须两个方向都闭合，否则显示值与持久值分叉（stale state）
 *   偏离处：两个 watch 理论上可回环，靠值判等截断；字符串判等可靠，不会死循环
 */
function bindModel(local, prop) {
  watch(local, v => emit(prop.event, v))
  watch(prop.get, v => {
    if (v !== local.value) local.value = v
  })
}

/**
 * 标题输入处理
 *
 * 输入 → 输出：input 事件 → 更新本地 ref + 立即 emit('update:title')
 * 关键机制：不走 bindModel 的 watch 链路，改为在 @input 里同步 emit，
 *           避免「本地 ref 已更新、父组件却没收到」的时序问题
 * 不变量：每次键盘输入后，父组件的 title 立即等于输入框的值
 * 边界：仅处理 text input；IME 组合期（中文拼音）中途也会 emit，但值一致，无副作用
 *
 * 依据：
 *   经典问题：受控组件数据流断裂（stale state / 单向数据流未闭合）
 *   本处变体：本地 ref 与 prop 同名，且更新经由 watch 异步排队，导致父组件读到旧值
 *   沿用结论：用户输入必须在事件回调内同步推给父组件，不能依赖异步 watcher
 *   偏离处：与 bindModel 相比多了一次显式赋值，换来确定性；字符串赋值幂等，无副作用
 */
function onTitleInput(e) {
  const v = e.target.value
  if (title.value !== v) title.value = v
  emit('update:title', v)
}

bindModel(title, { event: 'update:title', get: () => props.title })

// 正文同步（此前缺失：父组件异步回填正文时编辑区显示不出来，详见 expand()）
// 只在「编辑区还没被输入过」时回填，避免与用户的本地编辑互相覆盖。
let bodyDirty = false
watch(
  () => props.modelValue,
  v => {
    const el = bodyEl.value
    if (!el || bodyDirty) return
    if (el.innerText === String(v || '').trim()) return
    el.innerText = v || ''
  }
)

/** 纯文本长度（去掉 Markdown 标记），用于字数统计 */
const count = computed(() => plainText(props.modelValue).length)
const canSubmit = computed(() => count.value > 0 && count.value <= props.maxLength)

/** 把 Markdown 粗略还原成纯文本，用于长度统计 */
function plainText(md) {
  return String(md || '')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, '[图片]')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/<[^>]+>/g, '')
    .replace(/[*_~`>#]/g, '')
    .replace(/\n+/g, '\n')
    .trim()
}

/**
 * DOM → Markdown
 *
 * 思路：逐个遍历根节点的子节点，每种块级元素生成一段 Markdown；
 * 行内元素（b/i/u/s/a/img）由 inline() 递归处理。
 * 依据：
 *   经典问题：结构化数据序列化（DOM → 线性文本）
 *   本处变体：只支持编辑器工具栏能产生的有限标签集，无需通用 HTML 解析
 *   沿用结论：递归下降按节点类型分支，复杂度 O(n)，嵌套关系正确
 *   偏离处：不支持任意 HTML（如表格、代码块），粘贴外来富文本会降级为纯文本，
 *           这对"评论/发帖"场景是可接受的取舍
 */
function toMarkdown(root) {
  const blocks = []

  // 行内序列化
  function inline(node) {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent
    if (node.nodeType !== Node.ELEMENT_NODE) return ''

    const tag = node.tagName.toLowerCase()
    const child = () => Array.from(node.childNodes).map(inline).join('')

    switch (tag) {
      case 'br':
        return '\n'
      case 'b':
      case 'strong':
        return `**${child()}**`
      case 'i':
      case 'em':
        return `*${child()}*`
      case 's':
      case 'strike':
      case 'del':
        return `~~${child()}~~`
      case 'u':
        return `<u>${child()}</u>`
      case 'a':
        return `[${child()}](${node.getAttribute('href') || ''})`
      case 'img':
        return `![${node.getAttribute('alt') || '图片'}](${node.getAttribute('src') || ''})`
      case 'span': {
        // 视频占位块：还原成 Markdown 视频标记，否则保存时视频会丢失
        const vsrc = node.getAttribute?.('data-video-src')
        if (vsrc) {
          const label = node.querySelector?.('.ev-text')?.textContent?.trim() || '视频'
          return `![${label}](${vsrc})`
        }
        return child()
      }
      default:
        return child()
    }
  }

  // 块级序列化
  const blockOf = node => {
    const tag = node.tagName.toLowerCase()

    if (tag === 'ul' || tag === 'ol') {
      const items = Array.from(node.querySelectorAll(':scope > li'))
      return items
        .map((li, i) => {
          const text = Array.from(li.childNodes).map(inline).join('').trim()
          return tag === 'ol' ? `${i + 1}. ${text}` : `- ${text}`
        })
        .join('\n')
    }

    if (tag === 'blockquote') {
      return Array.from(node.childNodes)
        .map(inline)
        .join('')
        .split('\n')
        .map(l => `> ${l}`)
        .join('\n')
    }

    // div / p / 其他：按段落处理
    return Array.from(node.childNodes).map(inline).join('').trim()
  }

  Array.from(root.childNodes).forEach(node => {
    if (node.nodeType === Node.TEXT_NODE) {
      const t = node.textContent.trim()
      if (t) blocks.push(t)
      return
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return
    const text = blockOf(node)
    if (text) blocks.push(text)
  })

  return blocks.join('\n\n').trim()
}

/**
 * Markdown → HTML（toMarkdown 的逆过程），用于把已保存的正文回填到编辑区。
 *
 * 输入 → 输出：正文 Markdown 字符串 → 可直接赋给 contenteditable 的 HTML
 * 关键机制：先把整个文本 HTML 转义，再逐行按块规则（标题/列表/引用/图片/分隔线）
 *           套上标签，最后对行内 `**粗体**` / `*斜体*` / 链接做一次替换
 * 不变量：任何用户文本都不会被当作标签执行（先转义后加工），因此无 XSS 面
 * 边界：只覆盖 toMarkdown 能产出的 Markdown 子集（标题、列表、引用、粗斜体、链接、图片）；
 *       代码块、表格、嵌套列表在回填时降级成普通段落
 *
 * 依据：
 *   经典问题：序列化–反序列化往返（round-trip）与注入防护（输出编码）
 *   本处变体：不做通用 Markdown 解析，只保证本编辑器自身产出可安全回读
 *   沿用结论：先编码再结构化，结构字符全部来自自己的模板，攻击面为零
 *   偏离处：往返不是无损的（代码块/表格丢失），因为编辑器工具栏本就不产生它们，
 *           真正需要完整 Markdown 的场景走富文本编辑器之外的后台表单
 */
function markdownToHtml(md) {
  const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

  // 视频地址判断（与 markdown.js 一致）
  const isVideoSrc = u => /\.(mp4|webm|ogv|ogg|mov|m4v)(\?.*)?$/i.test(String(u || ''))

  const inlineHtml = s =>
    esc(s)
      // 图片必须先于链接处理：否则 `![alt](src)` 会先被链接规则吃掉，留下一个多余的 `!`
      // 视频例外：不能转成 <img>（浏览器会把 mp4 当图片，显示成一帧模糊画面），
      // 改成不可编辑的占位块，编辑时能一眼看出"这里是个视频"
      .replace(/!\[([^\]]*)\]\(([^)\s]+)\)/g, (full, alt, src) => {
        if (isVideoSrc(src)) {
          return (
            `<span class="editor-video" contenteditable="false" data-video-src="${src}">` +
            `<span class="ev-play">▶</span><span class="ev-text">${esc(alt || '视频')}</span></span>`
          )
        }
        return `<img src="${src}" alt="${alt}">`
      })
      .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, '<a href="$2">$1</a>')
      .replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>')
      .replace(/(^|[^*])\*([^*]+)\*/g, '$1<i>$2</i>')

  const out = []
  for (const raw of String(md || '').split(/\r?\n/)) {
    const line = raw.trimEnd()
    if (!line.trim()) continue
    let m
    if ((m = line.match(/^(#{1,6})\s+(.*)$/))) {
      out.push(`<h${m[1].length}>${inlineHtml(m[2])}</h${m[1].length}>`)
    } else if (/^\s*(-{3,}|\*{3,})\s*$/.test(line)) {
      out.push('<hr>')
    } else if (/^\s*[-*]\s+/.test(line)) {
      out.push(`<div>• ${inlineHtml(line.replace(/^\s*[-*]\s+/, ''))}</div>`)
    } else if (/^\s*\d+\.\s+/.test(line)) {
      out.push(`<div>${inlineHtml(line)}</div>`)
    } else if (/^\s*>\s?/.test(line)) {
      out.push(`<blockquote>${inlineHtml(line.replace(/^\s*>\s?/, ''))}</blockquote>`)
    } else if (/^\s*!\[/.test(line)) {
      out.push(`<div>${inlineHtml(line)}</div>`)
    } else {
      out.push(`<div>${inlineHtml(line)}</div>`)
    }
  }
  return out.join('')
}

function onInput() {
  if (!bodyEl.value) return
  bodyDirty = true
  emit('update:modelValue', toMarkdown(bodyEl.value))
  error.value = ''
}

function expand() {
  expanded.value = true
  nextTick(() => {
    // 回填正文：markdownToHtml 是 toMarkdown 的逆过程，保证已保存内容可再编辑
    if (bodyEl.value && !bodyEl.value.innerText.trim() && props.modelValue) {
      bodyEl.value.innerHTML = markdownToHtml(props.modelValue)
    }
    if (props.showTitle && titleEl.value) titleEl.value.focus()
    else bodyEl.value?.focus()
  })
}

/**
 * 进入编辑态（父组件异步拿到数据后调用）
 *
 * 为什么不用 props 单向流入：父组件在 onMounted 里 await 请求，
 * 子组件早已挂载完毕，此时改 props 不会自动刷新 contenteditable（同一 stale state 问题）。
 * 这里主动灌入 HTML，并清掉 bodyDirty，让后续 props 同步重新生效。
 */
async function edit(markdown, titleText) {
  expanded.value = true
  if (typeof titleText === 'string') title.value = titleText
  await nextTick()
  bodyDirty = false
  if (bodyEl.value) bodyEl.value.innerHTML = markdownToHtml(markdown)
  bodyEl.value?.focus()
}

function collapse() {
  expanded.value = false
  error.value = ''
}

function exec(cmd) {
  bodyEl.value?.focus()
  document.execCommand(cmd, false, null)
  onInput()
}

function insertList(kind) {
  bodyEl.value?.focus()
  document.execCommand(kind === 'ol' ? 'insertOrderedList' : 'insertUnorderedList')
  onInput()
}

// ==================== 表情 ====================

/** 点击贴图：插入 `:key:` 标记，渲染时由 markdown.js 替换成对应的内联 SVG */
function insertEmoji({ key }) {
  insertAtCursor(`[${key}]`)
  bodyDirty = true
  onInput()
}

// ==================== 图片 ====================

function pickImage() {
  fileEl.value?.click()
}

function onFilePick(e) {
  const files = Array.from(e.target.files || []).filter(f => f.type.startsWith('image/'))
  if (files.length) uploadImages(files)
  e.target.value = ''
}

function onPaste(e) {
  const items = e.clipboardData?.items
  if (!items) return
  const files = []
  for (const item of items) {
    if (item.type.startsWith('image/') || (item.type.startsWith('video/') && props.showVideo)) {
      const f = item.getAsFile()
      if (f) files.push(f)
    }
  }
  if (files.length) {
    e.preventDefault()
    const imgs = files.filter(f => f.type.startsWith('image/'))
    const vids = files.filter(f => f.type.startsWith('video/'))
    if (imgs.length) uploadImages(imgs)
    if (vids.length) uploadVideos(vids)
    return
  }

  // 粘贴的是纯文本：如果整段就是一张图片/视频的 Markdown 语法，
  // 直接渲染成内嵌元素，而不是留下一串 `![视频](url)` 源码。
  //
  // 依据：
  //   经典问题：序列化往返（parse → 内部表示 → 再序列化）
  //   本处变体：用户手打的 Markdown 片段要变成富文本节点，
  //             但同一份字符串还要能被 toMarkdown 原样还原
  //   沿用结论：只要 parse 与 serialize 的规则集一致，往返就无损
  //   偏离处：只处理"整段就是一个资源标记"的情形，混排文字交给浏览器默认插入
  const text = e.clipboardData?.getData('text/plain') || ''
  const m = text.trim().match(/^!\[([^\]]*)\]\(([^)\s]+)\)$/)
  if (m) {
    e.preventDefault()
    const [, alt, src] = m
    if (isVideoSrc(src)) {
      // 复用与渲染一致的占位结构，保证之后 toMarkdown 能还原
      const el = document.createElement('span')
      el.className = 'editor-video'
      el.setAttribute('contenteditable', 'false')
      el.setAttribute('data-video-src', src)
      el.innerHTML = `<span class="ev-play">▶</span><span class="ev-text">${alt || '视频'}</span>`
      insertNodeAtCursor(el)
    } else {
      const img = document.createElement('img')
      img.src = src
      img.alt = alt || ''
      insertNodeAtCursor(img)
    }
  }
}

/** 把节点插到当前光标处，并把光标移到节点后面 */
function insertNodeAtCursor(node) {
  const sel = window.getSelection()
  if (!sel || !sel.rangeCount) {
    bodyEl.value?.appendChild(node)
    onInput()
    return
  }
  const range = sel.getRangeAt(0)
  range.deleteContents()
  range.insertNode(node)
  range.setStartAfter(node)
  range.collapse(true)
  sel.removeAllRanges()
  sel.addRange(range)
  onInput()
}

function onDrop(e) {
  dragging.value = false
  const files = Array.from(e.dataTransfer?.files || [])
  const images = files.filter(f => f.type.startsWith('image/'))
  const videos = files.filter(f => f.type.startsWith('video/'))
  if (images.length) uploadImages(images)
  if (videos.length && props.showVideo) uploadVideos(videos)
}

// ==================== 视频 ====================

function pickVideo() {
  videoEl.value?.click()
}

function onVideoPick(e) {
  const files = Array.from(e.target.files || []).filter(f => f.type.startsWith('video/'))
  if (files.length) uploadVideos(files)
  e.target.value = ''
}

/**
 * 上传视频并插入播放器标记
 *
 * 输入 → 输出：视频文件列表 → 正文里插入 `![视频](url)` 形式的标记
 * 关键机制：复用与图片相同的 /api/upload 端点，前端只把标签换成 VideoPlayer
 * 边界：单个视频失败不影响后续文件；大文件超过后端上限时后端会返回明确错误
 *
 * 依据：
 *   经典问题：大文件传输的进度反馈（progressive upload feedback）
 *   本处变体：视频动辄几十 MB，没有进度条用户会以为卡死
 *   沿用结论：用 axios 的 onUploadProgress 回报百分比，失败逐个提示
 *   偏离处：不做分片/断点续传 —— 个人站点的视频量级是"偶尔传一个"，
 *           引入分片反而增加后端与存储的复杂度
 */
async function uploadVideos(files) {
  if (!props.showVideo) return
  uploading.value += files.length
  for (const file of files) {
    try {
      const form = new FormData()
      form.append('file', file)
      const res = await http.post('/api/upload', form, {
        headers: { 'Content-Type': 'multipart/form-data' },
        onUploadProgress: (ev) => {
          if (ev.total) videoProgress.value = Math.round((ev.loaded / ev.total) * 100)
        }
      })
      // 插入富文本占位块而不是纯文本：
      // contenteditable 里的文本节点永远是源码模样，只有真正的 DOM 节点才会渲染成标签
      insertVideoNode(res.url, '视频')
    } catch (e) {
      error.value = `视频上传失败：${e.message}`
    } finally {
      uploading.value -= 1
      videoProgress.value = 0
    }
  }
  bodyDirty = true
  onInput()
}

/** 上传并在光标处插入 Markdown 图片语法 */
async function uploadImages(files) {
  if (!props.showImage) return
  uploading.value += files.length
  for (const file of files) {
    try {
      const form = new FormData()
      form.append('file', file)
      const res = await http.post('/api/upload', form, {
        headers: { 'Content-Type': 'multipart/form-data' }
      })
      // 同视频：插 DOM 节点，不插 Markdown 文本
      insertImageNode(res.url, '图片')
    } catch (e) {
      error.value = `图片上传失败：${e.message}`
    } finally {
      uploading.value -= 1
    }
  }
  onInput()
}

/**
 * 记录编辑区最后的光标位置。
 * 点工具栏/表情面板时编辑区会失焦，选区随之丢失；插入前先恢复到这里，
 * 否则贴图会跑到正文最前面。
 */
function saveCaret() {
  const el = bodyEl.value
  if (!el) return
  const sel = window.getSelection()
  if (sel && sel.rangeCount && el.contains(sel.getRangeAt(0).commonAncestorContainer)) {
    savedRange = sel.getRangeAt(0).cloneRange()
  }
}

function onBodyBlur() {
  saveCaret()
}

function insertAtCursor(text) {
  const el = bodyEl.value
  if (!el) return
  el.focus()
  const sel = window.getSelection()

  // 优先用保存的选区（编辑器失去过焦点的情况）
  if (savedRange && sel) {
    sel.removeAllRanges()
    sel.addRange(savedRange)
  }

  if (!sel || sel.rangeCount === 0) {
    el.innerText += text
    return
  }
  const range = sel.getRangeAt(0)
  range.deleteContents()
  const node = document.createTextNode(text)
  range.insertNode(node)
  range.setStartAfter(node)
  range.collapse(true)
  sel.removeAllRanges()
  sel.addRange(range)
  savedRange = range.cloneRange()
}

/**
 * 在光标处插入一个视频占位块。
 *
 * 输入 → 输出：视频 url + 显示文案 → 编辑器里出现 `▶ 视频` 的不可编辑标签
 * 关键机制：插入真正的 DOM 元素而非文本。contenteditable 里的 Text 节点
 *           永远按字面显示，只有元素节点才会渲染成界面元素。
 * 不变量：该节点的 data-video-src 与 toMarkdown 的还原规则一一对应，
 *         保存后正文里仍是 `![视频](url)`，往返无损。
 * 边界：编辑器未聚焦时（savedRange 为空）追加到末尾。
 */
function insertVideoNode(url, label = '视频') {
  const wrap = document.createElement('div')
  const span = document.createElement('span')
  span.className = 'editor-video'
  span.setAttribute('contenteditable', 'false')
  span.setAttribute('data-video-src', url)
  span.innerHTML = `<span class="ev-play">▶</span><span class="ev-text">${label}</span>`
  wrap.appendChild(span)
  insertElementAtCursor(wrap)
}

/** 在光标处插入图片元素 */
function insertImageNode(url, alt = '图片') {
  const img = document.createElement('img')
  img.src = url
  img.alt = alt
  insertElementAtCursor(img)
}

/** 把元素插到保存的选区处，并把光标移到元素之后 */
function insertElementAtCursor(node) {
  const el = bodyEl.value
  if (!el) return
  el.focus()
  const sel = window.getSelection()
  if (savedRange && sel) {
    sel.removeAllRanges()
    sel.addRange(savedRange)
  }
  if (!sel || sel.rangeCount === 0) {
    el.appendChild(node)
    bodyDirty = true
    onInput()
    return
  }
  const range = sel.getRangeAt(0)
  range.deleteContents()
  range.insertNode(node)

  // 元素后面补一个换行，否则光标会卡在块内部、继续输入会粘进去
  const br = document.createElement('br')
  node.parentNode.insertBefore(br, node.nextSibling)
  const newRange = document.createRange()
  newRange.setStartAfter(br)
  newRange.collapse(true)
  sel.removeAllRanges()
  sel.addRange(newRange)
  savedRange = newRange.cloneRange()

  bodyDirty = true
  onInput()
}

function submit() {
  if (count.value > props.maxLength) {
    error.value = `内容超长（${count.value}/${props.maxLength}）`
    return
  }
  if (!canSubmit.value) return
  emit('submit', { title: title.value, content: props.modelValue })
}

onMounted(() => {
  if (props.autoFocus) expand()
})

defineExpose({ expand, collapse, edit })
</script>

<style scoped>
.editor {
  width: 100%;
}

/* ---------- 收起态 ---------- */
.collapsed {
  cursor: text;
}

.fake-input {
  padding: 0.75rem 1rem;
  background: #060a14;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: var(--text-muted);
  font-size: 0.9rem;
  transition: border-color 0.22s var(--ease), background 0.22s var(--ease);
}

.collapsed:hover .fake-input {
  border-color: rgba(76, 201, 240, 0.35);
  background: rgba(76, 201, 240, 0.03);
}

/* ---------- 展开态 ---------- */
.expanded-body {
  background: #060a14;
  border: 1px solid rgba(76, 201, 240, 0.3);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.title-input {
  width: 100%;
  padding: 0.75rem 1rem;
  background: transparent;
  border: none;
  border-bottom: 1px solid var(--border);
  color: var(--text-primary);
  font-size: 1rem;
  font-family: inherit;
  font-weight: 600;
}

.title-input:focus {
  outline: none;
}

.title-input::placeholder {
  color: var(--text-muted);
  font-weight: 400;
}

/* 标题为空且未聚焦时，给一条略明显的中性线，暗示"这行是标题，不是正文" */
.title-input:placeholder-shown {
  border-bottom-color: rgba(255, 255, 255, 0.22);
}

.title-input:focus {
  border-bottom-color: rgba(76, 201, 240, 0.5);
}

.body-wrap {
  position: relative;
}

.body-input {
  min-height: 9rem;
  max-height: 32rem;
  overflow-y: auto;
  padding: 0.85rem 1rem;
  color: var(--text-primary);
  font-size: 0.93rem;
  line-height: 1.75;
  outline: none;
  word-break: break-word;
}

.body-input:empty::before {
  content: attr(data-placeholder);
  color: var(--text-muted);
  pointer-events: none;
  opacity: 0.7;
}

/* 编辑器里的视频占位：不能是真播放器（contenteditable 里嵌 video 会互相干扰），
   用一个可识别的块，保存时再还原成 Markdown */
.body-input :deep(.editor-video) {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.28rem 0.7rem;
  margin: 0.15rem 0.1rem;
  background: rgba(76, 201, 240, 0.1);
  border: 1px solid rgba(76, 201, 240, 0.32);
  border-radius: 6px;
  color: #7ddaf7;
  font-size: 0.82rem;
  font-family: var(--font-mono);
  cursor: default;
  user-select: none;
}

.body-input :deep(.ev-play) {
  font-size: 0.72rem;
  opacity: 0.9;
}

.body-input :deep(.ev-text) {
  max-width: 16em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.body-input img {
  max-width: 100%;
  border-radius: var(--radius-sm);
  margin: 0.4rem 0;
  display: block;
}

.body-input a {
  color: var(--accent);
}

.drop-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(5, 7, 15, 0.85);
  border: 2px dashed var(--accent);
  border-radius: var(--radius-md);
  color: var(--accent);
  font-size: 0.9rem;
  font-family: var(--font-mono);
  letter-spacing: 0.06em;
  pointer-events: none;
}

.upload-hint {
  padding: 0.35rem 1rem;
  font-size: 0.76rem;
  color: var(--accent);
  font-family: var(--font-mono);
  border-top: 1px solid var(--border);
}

/* ---------- 工具栏 ---------- */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.35rem 0.6rem;
  border-top: 1px solid var(--border);
  background: rgba(76, 201, 240, 0.03);
}

.tools-left {
  display: flex;
  align-items: center;
  gap: 0.15rem;
  flex-wrap: wrap;
}

.tool {
  min-width: 1.9rem;
  height: 1.9rem;
  padding: 0 0.35rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: 0.85rem;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.16s var(--ease);
  user-select: none;
}

.tool:hover {
  background: rgba(76, 201, 240, 0.12);
  border-color: rgba(76, 201, 240, 0.28);
  color: var(--accent);
}

.tool:active {
  transform: scale(0.94);
}

/* 表情按钮的高亮态（面板打开时保持亮着，让用户知道是谁开的） */
.tool.active {
  background: rgba(76, 201, 240, 0.16);
  border-color: rgba(76, 201, 240, 0.4);
  color: var(--accent);
}

/* 工具栏里的 SVG 图标：与文字按钮视觉重量对齐 */
.tool-svg {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.tool-svg :deep(svg) {
  display: block;
}

/* 表情面板的定位锚点：面板用 absolute 挂在按钮上方 */
.tool-emoji {
  position: relative;
  display: inline-flex;
}

.tool-sep {
  width: 1px;
  height: 1.05rem;
  background: var(--border);
  margin: 0 0.3rem;
}

.counter {
  font-family: var(--font-mono);
  font-size: 0.7rem;
  color: var(--text-muted);
  flex-shrink: 0;
}

.counter.over {
  color: var(--danger);
}

.hidden-file {
  display: none;
}

/* ---------- 操作栏 ---------- */
.actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.6rem;
  margin-top: 0.7rem;
}

.btn-cancel {
  padding: 0.45rem 1.05rem;
  background: transparent;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: 0.85rem;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.2s var(--ease);
}

.btn-cancel:hover {
  background: rgba(126, 156, 210, 0.08);
  color: var(--text-primary);
}

.btn-publish {
  padding: 0.45rem 1.4rem;
  background: linear-gradient(135deg, #2b7fd4, #6a5ae0);
  border: none;
  border-radius: var(--radius-sm);
  color: #fff;
  font-size: 0.85rem;
  font-weight: 500;
  font-family: inherit;
  letter-spacing: 0.03em;
  cursor: pointer;
  transition: all 0.22s var(--ease);
}

.btn-publish:hover:not(:disabled) {
  background: linear-gradient(135deg, #3a92e8, #7b6cf6);
  box-shadow: 0 0 20px rgba(106, 90, 224, 0.42);
}

.btn-publish:disabled {
  opacity: 0.42;
  cursor: not-allowed;
}

.err {
  margin-top: 0.45rem;
  font-size: 0.78rem;
  color: #fca5a5;
}
</style>
