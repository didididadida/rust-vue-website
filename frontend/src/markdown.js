// 统一的 Markdown 渲染入口。
// 代码块用 highlight.js 做语法高亮：每种语言有各自的词法规则与配色（见 style.css 里的 .hljs-*）。
// 这里只按需注册用得到的语言，避免把 highlight.js 全量打包进来。

import { emojiImg } from './emoji'
import { createApp } from 'vue'
import { Marked } from 'marked'
import hljs from 'highlight.js/lib/core'

import bash from 'highlight.js/lib/languages/bash'
import c from 'highlight.js/lib/languages/c'
import cpp from 'highlight.js/lib/languages/cpp'
import css from 'highlight.js/lib/languages/css'
import ini from 'highlight.js/lib/languages/ini' // INI / TOML
import javascript from 'highlight.js/lib/languages/javascript'
import json from 'highlight.js/lib/languages/json'
import python from 'highlight.js/lib/languages/python'
import rust from 'highlight.js/lib/languages/rust'
import sql from 'highlight.js/lib/languages/sql'
import typescript from 'highlight.js/lib/languages/typescript'
import xml from 'highlight.js/lib/languages/xml'
import yaml from 'highlight.js/lib/languages/yaml'

const LANGUAGES = {
  bash,
  c,
  cpp,
  css,
  ini,
  javascript,
  json,
  python,
  rust,
  sql,
  typescript,
  xml,
  yaml,
}

Object.entries(LANGUAGES).forEach(([name, def]) => hljs.registerLanguage(name, def))

// 代码块右上角显示的名字（highlight.js 自带的 name 有长有短，这里自己控制）
const DISPLAY_NAME = {
  py: 'Python',
  python: 'Python',
  c: 'C',
  h: 'C',
  cpp: 'C++',
  'c++': 'C++',
  bash: 'Bash',
  sh: 'Bash',
  shell: 'Bash',
  zsh: 'Bash',
  js: 'JavaScript',
  javascript: 'JavaScript',
  ts: 'TypeScript',
  typescript: 'TypeScript',
  json: 'JSON',
  toml: 'TOML',
  ini: 'INI',
  yaml: 'YAML',
  yml: 'YAML',
  rust: 'Rust',
  rs: 'Rust',
  xml: 'XML',
  html: 'HTML',
  css: 'CSS',
  sql: 'SQL',
}

const escapeHtml = (text) =>
  String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')

const COPY_ICON =
  '<svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true">' +
  '<rect x="5.5" y="5.5" width="9" height="9.5" rx="1.5"/>' +
  '<path d="M10.5 5.5V3A1.5 1.5 0 0 0 9 1.5H3A1.5 1.5 0 0 0 1.5 3v6A1.5 1.5 0 0 0 3 10.5h2.5"/>' +
  '</svg>'

const COPY_BUTTON =
  `<button type="button" class="code-copy" title="复制代码" aria-label="复制代码">` +
  `${COPY_ICON}<span class="code-copy-text">复制</span></button>`

/// 复制文本：优先用剪贴板 API，不可用时回退到 execCommand
async function copyText(text) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch {
      /* 无权限 / 非安全上下文，走回退方案 */
    }
  }
  try {
    const area = document.createElement('textarea')
    area.value = text
    area.setAttribute('readonly', '')
    area.style.position = 'fixed'
    area.style.top = '-1000px'
    document.body.appendChild(area)
    area.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(area)
    return ok
  } catch {
    return false
  }
}

/// 在 root 上启用「一键复制」（事件委派，适配 v-html 渲染的内容）
/// 返回解绑函数
const copyTimers = new WeakMap()

function enableCodeCopy(root = document) {
  const onClick = async (event) => {
    const target = event.target
    if (!(target instanceof Element)) return
    const button = target.closest('.code-copy')
    if (!button || !root.contains(button)) return

    const code = button.closest('.code-frame')?.querySelector('pre code')
    if (!code) return

    event.preventDefault()
    const label = button.querySelector('.code-copy-text')
    const ok = await copyText(code.textContent)

    button.classList.toggle('is-copied', ok)
    if (label) label.textContent = ok ? '已复制' : '复制失败'

    clearTimeout(copyTimers.get(button))
    copyTimers.set(
      button,
      setTimeout(() => {
        button.classList.remove('is-copied')
        if (label) label.textContent = '复制'
      }, 1600),
    )
  }

  root.addEventListener('click', onClick)
  return () => root.removeEventListener('click', onClick)
}

const marked = new Marked({ gfm: true })

/**
 * 自定义渲染器：
 *   - 代码块 → 高亮 + 复制按钮
 *   - 视频链接（`![视频](x.mp4)` 或裸的 `<video>`）→ 原生播放器
 *   - 表情标记 `:key:` → 内联 SVG 贴图
 *
 * 依据：
 *   经典问题：Markdown 扩展点（renderer / tokenizer 的插拔）
 *   本处变体：表情和视频都不是标准 Markdown 语法，但正文是 Markdown 存储的
 *   沿用结论：扩展渲染器而不改 Markdown 源文本，存储格式向后兼容
 *   偏离处：表情用 `:key:` 这类短标记而非 HTML，纯文本降级时仍可读；
 *           渲染时若 key 不存在就原样输出，不会吞掉用户文本
 */
marked.use({
  renderer: {
    // marked v12+ 的 renderer 收到的是 token 对象
    code({ text, lang }) {
      const name = String(lang || '')
        .trim()
        .split(/\s+/)[0]
        .toLowerCase()

      // 没写语言（比如目录树、纯文本示意）就保持朴素样式，不做高亮
      if (!name) {
        return `<pre><code>${escapeHtml(text)}</code></pre>`
      }

      const known = hljs.getLanguage(name)
      const body = known
        ? hljs.highlight(text, { language: name, ignoreIllegals: true }).value
        : escapeHtml(text)
      const label = DISPLAY_NAME[name] || name
      const cls = known ? ` class="hljs language-${name}"` : ''

      return (
        `<div class="code-frame">` +
        `<div class="code-bar"><span class="code-lang">${escapeHtml(label)}</span>${COPY_BUTTON}</div>` +
        `<pre><code${cls}>${body}</code></pre>` +
        `</div>`
      )
    },

    /**
     * 段落：处理"整段只有一个视频"的情况。
     * 编辑器插入的视频标记单独成段，这里把它升级成播放器；
     * 段落里混了文字时保持普通段落，避免把正文拆散。
     */
    paragraph({ tokens }) {
      const inline = this.parser.parseInline(tokens)
      const onlyVideo = tokens.length === 1 && tokens[0].type === 'image' && isVideoToken(tokens[0])
      if (onlyVideo) {
        return `<div class="video-frame">${renderVideo(tokens[0].href, tokens[0].text)}</div>`
      }
      return `<p>${inline}</p>`
    },
  },
})

/** 判断图片标记是不是视频：扩展名在视频白名单里，或 alt 写成「视频」 */
const VIDEO_EXT = /\.(mp4|webm|ogv|ogg|mov|m4v)(\?.*)?$/i

function isVideoToken(token) {
  if (!token || typeof token.href !== 'string') return false
  return VIDEO_EXT.test(token.href) || String(token.text || '').trim() === '视频'
}

/**
 * 渲染视频元素（原生 <video>）。
 *
 * 只放行站内相对路径与 http(s)，拒绝 javascript: 之类的伪协议。
 */
function renderVideo(src, poster) {
  const url = String(src || '')
  if (!/^(\/|https?:\/\/)/i.test(url)) return ''
  const posterAttr =
    poster && /^(\/|https?:\/\/)/i.test(poster)
      ? ` poster="${escapeHtml(poster)}"`
      : ''
  return `<video class="video-native" src="${escapeHtml(url)}"${posterAttr} controls preload="metadata"></video>`
}

/**
 * 把 `:key:` 表情标记替换成内联 SVG。
 *
 * 输入 → 输出：已渲染的 HTML → 贴图替换后的 HTML
 * 关键机制：在 HTML 上做替换，但只替换 `:key:` 且 key 在白名单里；
 *           替换内容是我们自己生成的 SVG，不含用户可控文本
 * 不变量：未知 key 原样保留；代码块里的标记不替换（先切出 <pre> 再处理其余部分）
 * 边界：`:key:` 出现在 URL 里时可能误判，但 `:` 不能出现在本项目的 URL 中，可接受
 *
 * 依据：
 *   经典问题：文本与标记的边界（escaping vs markup）
 *   本处变体：替换发生在渲染后的 HTML 上，而不是 Markdown 源上
 *   沿用结论：只对白名单 key 做替换，等价于"安全的模板替换"，不引入注入面
 *   偏离处：不做上下文感知的解析器扩展，理由是用例单一且 key 集固定
 */
function applyEmoji(html) {
  // 先保护代码块：把 <pre>...</pre> 原样抽走，处理完再放回
  const blocks = []
  const withHoles = String(html).replace(/<pre[\s\S]*?<\/pre>/g, m => {
    blocks.push(m)
    return `\u0000PRE${blocks.length - 1}\u0000`
  })

  // `[doge]` 或 `:doge:` 两种写法都支持（B 站用方括号，旧数据用冒号）
  const replaced = withHoles.replace(/\[([a-z0-9-]+)\]|:([a-z0-9-]+):/gi, (full, sqKey, colonKey) => {
    const img = emojiImg(sqKey || colonKey)
    return img || full
  })

  return replaced.replace(/\u0000PRE(\d+)\u0000/g, (_, i) => blocks[Number(i)] || '')
}

/// 把 Markdown 渲染成 HTML 字符串
export function renderMarkdown(markdown) {
  const html = marked.parse(markdown ?? '')
  return applyEmoji(html)
}

// ============================================================================
// XSS 净化：所有进入 v-html 的 HTML 都必须先过这里
// ============================================================================
//
// 问题：marked 默认放行原始 HTML。`marked.parse("<img src=x onerror=alert(1)>")`
//       的原样输出，经 v-html 插入就会执行。之前全站 Markdown 输出没有任何净化。
//
// 输入 → 输出：marked 渲染出的 HTML 字符串 → 只含安全标签/属性的 HTML 字符串
// 关键机制：DOMPurify 按「标签 + 属性」白名单裁剪，
//           - 脚本、iframe、object、embed 等直接剪掉（KEEP_CONTENT=false 连内容一起删）
//           - 只允许 http/https/mailto、站内相对路径、data:image 这几类 URL，
//             于是 javascript: 伪协议被挡在 href/src 之外
//           - 允许 data-* 属性，因为视频占位（data-video-src）靠它工作
// 不变量：净化后仍包含全部「正文语义」标签与代码高亮类名（pre/code/span.hljs-*）
// 边界：DOMPurify 依赖浏览器 DOM，SSR/Node 下不可用 —— 本项目纯 CSR，不涉及；
//       若将来做 SSR，需要对同一份 HTML 打两套策略并做差分测试。
//
// 依据：
//   经典问题：不可信输入的「输出编码」（output encoding / contextual escaping）
//   本处变体：输出目标是 HTML 字符串而非文本；且净化发生在 Markdown 渲染之后，
//             因此既有用户写的 Markdown，也有 Markdown 里内嵌的原始 HTML
//   沿用结论：白名单式净化在渲染末端做一次，等价于对所有上下文统一编码——
//             所有 v-html 调用点自动受保护，不依赖每个调用方记得转义
//   偏离处：白名单会剪掉「用户想嵌入的合法富文本」（如自己写的 <iframe>）；
//           本项目的富文本能力已由编辑器 + 自定义渲染器覆盖，这个损失可接受
const PURIFY_CONFIG = {
  ADD_ATTR: ['target', 'rel', 'data-video-src', 'data-video-poster', 'data-route'],
  // 代码高亮与视频占位依赖 class / data-*，DOMPurify 默认允许 class，这里显式保留 data
  ALLOW_DATA_ATTR: true,
  // 剪掉标签时一并剪掉内容，避免脚本正文以纯文本形式漏到页面上
  KEEP_CONTENT: false,
  // 只允许安全的 URL 协议
  ALLOWED_URI_REGEXP:
    /^(?:(?:https?|mailto):|\/(?!\/)|\.{0,2}\/|#|data:image\/(?:png|jpe?g|gif|webp);)/i,
}

/** 净化一段 HTML。DOMPurify 在模块首次调用时懒加载。 */
let purify = null
let purifyFailed = false
async function ensurePurify() {
  if (purify || purifyFailed) return purify
  try {
    const mod = await import('dompurify')
    purify = mod.default || mod
  } catch (e) {
    // 加载失败时不能静默放行：退化为「只保留纯文本」，宁可少显示也不执行脚本
    purifyFailed = true
    console.error('DOMPurify 加载失败，已降级为纯文本输出:', e)
  }
  return purify
}

/**
 * 异步净化（推荐入口）。
 * @param {string} html
 * @returns {Promise<string>}
 */
export async function sanitizeHtml(html) {
  const p = await ensurePurify()
  if (!p) return escapeHtml(html) // 降级：纯文本化，杜绝脚本执行
  return p.sanitize(String(html ?? ''), PURIFY_CONFIG)
}

/**
 * 同步净化：DOMPurify 已加载时直接用；尚未加载时返回「纯文本化」的结果。
 * 给 computed 这类同步上下文使用，首次渲染可能少显示富文本，加载完成后会重渲染。
 * 因此启动时应在 main.js 里调用 `warmupSanitizer()` 提前加载。
 */
export function sanitizeHtmlSync(html) {
  if (purify) return purify.sanitize(String(html ?? ''), PURIFY_CONFIG)
  return escapeHtml(html)
}

/** 启动时预热，保证首屏渲染就能用上真正的净化而不是降级分支 */
export function warmupSanitizer() {
  return ensurePurify()
}

export { enableCodeCopy }
export default marked
