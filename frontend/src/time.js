// 时间与数字的展示工具（社区/帖子/动态卡片共用）
//
// 依据：
//   经典问题：显示格式的一致性（单一职责）
//   本处变体：相对时间在多处卡片上出现，且同一页面常有十几个时间戳
//   沿用结论：把"多久以前"和"计数缩写"收敛成纯函数，各页面调用同一实现
//   偏离处：不使用 Date 的 Intl.RelativeTimeFormat，避免浏览器版本差异导致文案不统一

/** 相对时间：刚刚 / N 分钟前 / N 小时前 / N 天前 / 具体日期 */
export function relativeTime(value) {
  if (!value) return ''
  const t = new Date(value).getTime()
  if (Number.isNaN(t)) return value
  const min = Math.floor((Date.now() - t) / 60000)
  if (min < 1) return '刚刚'
  if (min < 60) return `${min} 分钟前`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr} 小时前`
  const day = Math.floor(hr / 24)
  if (day < 30) return `${day} 天前`
  return new Date(t).toLocaleDateString('zh-CN')
}

/** 计数缩写：999 / 1.2万 / 12.3万，B 站同款 */
export function compact(n) {
  const v = Number(n || 0)
  if (!Number.isFinite(v)) return '0'
  if (v < 10000) return String(v)
  return (v / 10000).toFixed(v >= 100000 ? 0 : 1).replace(/\.0$/, '') + '万'
}

/** 完整时间：2026/11/16 05:21 */
export function formatDateTime(v) {
  if (!v) return ''
  const d = new Date(v)
  if (Number.isNaN(d.getTime())) return v
  return d.toLocaleString('zh-CN', {
    year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit'
  })
}

/**
 * 头像配色的默认色板：冷色 + 低饱和，杜绝粉红/暗红这类刺眼颜色。
 *
 * 依据：最小惊讶原则 —— 头像只是身份标识，不该抢正文的注意力；
 *       深色主题下高饱和粉红会形成视觉噪点，也容易和"点赞/删除"等语义色混淆。
 * 变体：仍是按用户名做稳定哈希，同一个人永远同一个颜色，只是色板换了。
 */
const AVATAR_PALETTE = [
  { from: '#4a7ba7', to: '#2f4f6f' }, // 雾蓝
  { from: '#5b7c99', to: '#35495e' }, // 灰蓝
  { from: '#4f8a8b', to: '#2f5a5b' }, // 青灰
  { from: '#6b8f71', to: '#3f5a45' }, // 苔绿
  { from: '#7a6f9b', to: '#4a4263' }, // 灰紫
  { from: '#8a7f6d', to: '#57503f' }, // 灰褐
  { from: '#3f7f9f', to: '#284f63' }, // 深海
  { from: '#6b7fa3', to: '#414e66' }, // 石板
]

/** 用户名 → 稳定的头像渐变色 */
export function avatarStyle(name) {
  let sum = 0
  for (const ch of String(name || '')) sum = (sum + ch.codePointAt(0)) % 9973
  const c = AVATAR_PALETTE[sum % AVATAR_PALETTE.length]
  return { background: `linear-gradient(140deg, ${c.from}, ${c.to})` }
}

/** 名字首字母/首字（中文取第一个字，英文取大写首字母） */
export function avatarInitial(name) {
  const s = String(name || '').trim()
  if (!s) return '?'
  const first = [...s][0]
  return /[a-z]/i.test(first) ? first.toUpperCase() : first
}

/**
 * 从用户粘贴的任意文本里提取出一个"纯 URL"。
 *
 * 输入 → 输出：粘贴的文本 → 裸链接；提取不到返回空串
 * 关键机制：按"最具体 → 最宽松"的顺序依次尝试：
 *   1. Markdown 图片  `![alt](url "title")`
 *   2. Markdown 链接  `[text](url "title")`
 *   3. HTML            `<img src="url">` / `<a href="url">`
 *   4. 裸链接          http(s)://… 或 /static/…
 * 不变量：返回值一定是可用的地址（去掉包裹的引号/尖括号/尾随标点），否则返回空串
 * 边界：一段文本里有多个链接时取第一个；相对路径只在 `/` 开头时接受
 *
 * 依据：
 *   经典问题：序列化往返的弱化版本 —— 只做"反解析出中心数据"（extract, not parse）
 *   本处变体：用户粘贴的可能来自图床的「复制 Markdown」，也可能来自浏览器地址栏、
 *             聊天软件（带 < > 包裹）或网页右键复制（HTML）
 *   沿用结论：先尝试结构化的写法，再退化为裸链接扫描，覆盖面最大且不会误伤
 *   偏离处：不做完整 Markdown 解析（不需要 alt/title），只取 URL 一个字段
 */
export function extractUrl(input) {
  const text = String(input || '').trim()
  if (!text) return ''

  // 1/2. Markdown 图片或链接：取 () 里的内容，再从中取出 URL（可能被 <> 包裹，也可能带标题）
  //      `![alt](url "title")` → 先用宽松匹配拿到括号内文本，再单独取第一个 token，
  //      这样 URL 里带空格（未编码）时也能尽量保住。
  const md = text.match(/!?\[[^\]]*\]\(([^)]*)\)/)
  if (md) {
    const inner = md[1].trim()
    const angle = inner.match(/^<([^>]*)>/)
    const picked = angle ? angle[1] : inner.split(/\s+/)[0]
    if (picked) return picked
  }

  // 3. HTML 属性
  const html = text.match(/(?:src|href)\s*=\s*["']([^"']+)["']/i)
  if (html && html[1]) return html[1].trim()

  // 4. 裸链接：http(s) 优先，其次站内绝对路径
  const bare = text.match(/https?:\/\/[^\s"'<>()]+/i)
  if (bare) return stripTrailingPunct(bare[0])

  const sitePath = text.match(/\/static\/[^\s"'<>()]+/)
  if (sitePath) return stripTrailingPunct(sitePath[0])

  return ''
}

/** 去掉链接末尾常见的标点（用户从句子中复制时经常带上来） */
function stripTrailingPunct(url) {
  return url.replace(/[.,;:!?、。，；：！？]+$/, '')
}
