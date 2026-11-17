// B 站官方表情包（acg.tv / bilibili 自带表情）
//
// 资源来源：https://github.com/YiJio/emoji-chinese 的 `bz/def` 目录
//   - 94 个 96×96 PNG，与 B 站网页版评论区使用的是同一套图
//   - 已下载到 public/emojis/bilibili/，由站点自己托管，不依赖外部 CDN
//   - 版权归哔哩哔哩所有，本站仅作个人学习用途
//
// 使用方式：
//   正文里写 `[doge]`，渲染时替换成 <img class="bili-emoji" src="/emojis/bilibili/doge.png">
//   编辑器表情面板也从这张表生成

/// 表情 key → 中文名（用于 tooltip 与表情面板的分组说明）
/// key 与 PNG 文件名一致
export const BILI_EMOJI = {
  // —— 常用 ——
  doge: '狗头',
  'doge-jg': '狗头-举牌',
  'doge-td': '狗头-舔',
  dd: 'dd',
  dd2: 'dd2',
  aj: '阿娇',
  ax: '啊谢',
  bf: '抱夫',
  bq: '抱歉',
  by: '白眼',
  cg: '吃瓜',
  cy: '擦眼泪',
  dai: '呆',
  dcall: '打电话',
  dk: 'добрый',
  dragon: '龙',
  dx: '大笑',
  dz: '癫',
  fby: '扶宝仪',
  fd: '奋斗',
  fdl: '发抖了',
  fu: '扶',
  gb: '鼓掌',
  gg: '跪了',
  gl: '交个朋友吧',
  gwl: '够我乐',
  gxx: '高兴兴',
  gz: '鼓掌',
  hd: '花呆',
  hg: '花哔',
  hj: '滑稽',
  hq: '哈欠',
  hu: '😯',
  hx: '花心',
  hyq: '汗一个',
  'hyq-jg': '汗一个-举牌',
  jl: '流泪',
  jt: '🐔',
  jx: '滑稽',
  jy: '惊讶',
  jy2: '惊讶2',
  kb: '抠鼻',
  kgz: '🦐',
  kz: '困',
  leng: '冷',
  lh: '灵魂',
  lhcq: '老花厨师',
  lyj: '流泪了',
  ma: '骂',
  mj: 'emmm',
  ncgb: '你查个播',
  ng: '嗯嗯',
  oh: 'Oh',
  ok: 'OK',
  ox: 'OX',
  pz: '盘子',
  rabbit: '兔',
  rat: '鼠',
  re: '热',
  sb: '坏',
  sd: '生气',
  sk: '思考',
  sl: '色',
  sl2: '色2',
  spwx: '生僻微笑',
  sq: '耸肩',
  'sq-jg': '耸肩-举牌',
  sxd: '双手叉腰',
  teng: '疼',
  tp: '托瓶',
  tx: '吐舌',
  ty: '摊手',
  wl: '无语',
  wq: '委屈',
  wx: '微笑',
  wy: '无语',
  wy2: '无语2',
  wz: '委屈哭',
  xgllbj: '小恐龙',
  xh: '心',
  xh2: '心2',
  xiao: '笑',
  xjeq: '修仙二祺',
  xk: '笑哭',
  xq: '星球',
  xs: '嘘',
  xxy: '小星星',
  xz: '醒着',
  yb: '右抱',
  yh: '晕乎',
  yx: '有病',
  zc: '嘴馋',
  zj: '遵纪',
  zk: '张开',
}

/// 图床基础路径
export const EMOJI_BASE = '/emojis/bilibili'

/// 取某个 key 的图片地址
export function emojiSrc(key) {
  return BILI_EMOJI[key] ? `${EMOJI_BASE}/${key}.png` : ''
}

/// 取中文名
export function emojiName(key) {
  return BILI_EMOJI[key] || key
}

/// 所有 key（按字典序，供表情面板遍历）
export const EMOJI_KEYS = Object.keys(BILI_EMOJI).sort()

/// 生成 <img> 标签（用于 Markdown 渲染结果的后处理）
export function emojiImg(key, size = 24) {
  const src = emojiSrc(key)
  if (!src) return ''
  const name = emojiName(key)
  return (
    `<img class="bili-emoji" src="${src}" alt="[${name}]" title="${name}" ` +
    `width="${size}" height="${size}" loading="lazy" draggable="false">`
  )
}

// ============================================================================
// 界面图标（不是表情）——用内联 SVG，因为这些是 UI 控件图标而非表情图
// ============================================================================

const ICONS = {
  smile:
    '<circle cx="12" cy="12" r="9"/><path d="M8.5 14.5s1.2 1.8 3.5 1.8 3.5-1.8 3.5-1.8"/><circle cx="9" cy="9.8" r="1"/><circle cx="15" cy="9.8" r="1"/>',
  image:
    '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8.5" cy="9.5" r="1.6"/><path d="M21 16l-5.2-5.2a1.6 1.6 0 0 0-2.3 0L4 20"/>',
  video:
    '<rect x="2.5" y="5.5" width="13" height="13" rx="2"/><path d="M15.5 10.2l5.2-3v9.6l-5.2-3z"/>',
  danmaku:
    '<rect x="2.5" y="4.5" width="19" height="12" rx="2.5"/><path d="M6.5 9h5M6.5 12.2h8"/><path d="M9 16.5V20l3-3"/>',
  like:
    '<path d="M7 10.5v9H4.2a1.2 1.2 0 0 1-1.2-1.2v-6.6A1.2 1.2 0 0 1 4.2 10.5H7z"/><path d="M7 10.5l3.8-7a2 2 0 0 1 2.9 2.3l-1 4.2h4.8a2 2 0 0 1 1.9 2.6l-1.8 5.6a2 2 0 0 1-1.9 1.3H7z"/>',
  comment:
    '<path d="M20.5 11.5a7.5 7.5 0 0 1-10.9 6.7L4 20l1.4-4.3A7.5 7.5 0 1 1 20.5 11.5z"/>',
  play: '<path d="M7 4.5l12 7.5-12 7.5z"/>',
  star:
    '<path d="M12 3.2l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17.2l-5.4 2.9 1-6.1L3.2 9.7l6.1-.9z"/>',
  lock:
    '<rect x="4.5" y="10" width="15" height="10.5" rx="2"/><path d="M7.8 10V7.5a4.2 4.2 0 0 1 8.4 0V10"/>',
  warn:
    '<path d="M12 3.5l9 16.5H3z"/><path d="M12 9.5v4.5"/><circle cx="12" cy="16.8" r=".8"/>',
  bell:
    '<path d="M6 9.5a6 6 0 1 1 12 0c0 4 1.5 5.5 1.5 5.5H4.5S6 13.5 6 9.5z"/><path d="M9.8 18.5a2.4 2.4 0 0 0 4.4 0"/>',
  trash:
    '<path d="M4 6.5h16"/><path d="M9 6.5V4.2h6v2.3"/><path d="M6.2 6.5l1 14h9.6l1-14"/>',
  send: '<path d="M21 3L10.5 13.5"/><path d="M21 3l-6.8 18-3.7-7.5L3 9.8z"/>',
  close: '<path d="M6 6l12 12M18 6L6 18"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  user:
    '<circle cx="12" cy="8" r="4"/><path d="M4.5 20.5a7.5 7.5 0 0 1 15 0"/>',
  chevron: '<path d="M8 10l4 4 4-4"/>',
  edit:
    '<path d="M4 20h4L20 8a2.8 2.8 0 0 0-4-4L4 16z"/><path d="M14 6l4 4"/>',
  settings:
    '<circle cx="12" cy="12" r="3"/><path d="M12 2.8v2.4M12 18.8v2.4M4.6 7.3l2 1.2M17.4 15.5l2 1.2M4.6 16.7l2-1.2M17.4 8.5l2-1.2"/>',
}

/// 生成界面图标的 SVG 字符串
export function iconSvg(name, size = 16) {
  const body = ICONS[name]
  if (!body) return ''
  return (
    `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" ` +
    `stroke="currentColor" stroke-width="1.7" stroke-linecap="round" ` +
    `stroke-linejoin="round" aria-hidden="true">${body}</svg>`
  )
}

/// 表情的中文名（替代旧 API，保留以兼容现有调用）
export function emojiChar(key) {
  return emojiName(key)
}

/// 按分组返回表情面板数据（供 EmojiPicker 使用）
export const GROUPS = [
  {
    key: 'bilibili',
    name: 'B站表情',
    emojis: EMOJI_KEYS.map(k => ({ key: k, name: emojiName(k), src: emojiSrc(k) })),
  },
]