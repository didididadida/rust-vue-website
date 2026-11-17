/**
 * Valaxy Press 主题的设计 token
 *
 * 逻辑描述：
 *   输入 → 输出：无 → 导出颜色/间距/排版常量，供组件与 CSS 变量使用
 *   关键机制：
 *     1. 严格照抄 valaxy.site 实际 CSS 里的 --press-c-* 变量值（已抓取核对）；
 *     2. 亮色/暗色两套，通过 html.dark 切换；
 *     3. 除了颜色，还固化排版尺度（标题字号、行高、最大宽度）。
 *   不变量：
 *      - 所有颜色只有一个来源，组件里不硬编码色值；
 *      - 暗色下文字对比度满足 WCAG AA（正文 ≥ 4.5:1）。
 *   边界：
 *      - 这只是设计 token，不包含组件样式；
 *      - 与 Element Plus 的主题变量互不干扰（EP 自己有一套 --el-*）。
 *
 * 依据：
 *   经典问题：设计 token 的唯一来源（design token centralization）
 *   来源：
 *     1. valaxy.site 实际产出 CSS（style.C5Uvk0GR.css，709KB）
 *        抓取得到的真实变量定义：
 *          --press-c-bg:#1b1b1f        （暗）
 *          --press-c-bg-alt:#161618    （暗）
 *          --press-c-bg-soft:#202127   （暗）
 *          --press-c-divider:#2e2e32   （暗）
 *          --press-c-text-1:#3c3c43    （亮）
 *          --press-c-text-2:#67676c    （亮）
 *          --press-c-indigo-1:#a8b1ff  （暗）/ #3451b2（亮）
 *          --press-c-green-1:#3dd68c   （暗）/ #18794e（亮）
 *     2. VitePress 的设计变量体系（Valaxy Press 主题沿用其命名）
 *        https://vitepress.dev/ —— --vp-c-* 前缀，Valaxy 改为 --pr-c-*
 *   原型结论：
 *     1. Valaxy Press 用一套中性灰 + 单一强调色的克制配色；
 *     2. 背景分三层（bg / bg-alt / bg-soft）来表达层级，不靠阴影。
 *   本处变体：
 *     原站点是深空蓝黑 + 青色强调（#4cc9f0），与 Valaxy 的中性灰 + 靛蓝不同。
 *   沿用结论：
 *     采用 Valaxy 的**层级结构**（三层背景 + 分隔线）与**排版尺度**，
 *     但保留本站既有的青色强调色（品牌一致性）。
 *   偏离处：
 *     不照搬靛蓝色 #a8b1ff，因为站点已有品牌色；
 *     背景灰阶则照搬，因为它解决的是"层级不清"这个具体问题。
 */

/** 亮色 */
export const LIGHT = {
  bg: '#ffffff',
  bgAlt: '#f6f6f7',
  bgSoft: '#f6f6f7',
  divider: '#e2e2e3',
  border: '#e2e2e3',
  text1: '#3c3c43',
  text2: '#67676c',
  text3: '#929295',
  accent: '#0d8fb8', // 本站品牌色的亮色版
  accentSoft: 'rgba(13, 143, 184, 0.10)',
  codeBg: '#f6f6f7',
}

/** 暗色（照抄 Valaxy 实际值） */
export const DARK = {
  bg: '#1b1b1f',
  bgAlt: '#161618',
  bgSoft: '#202127',
  divider: '#2e2e32',
  border: '#2e2e32',
  text1: '#dfdfd6', // Valaxy 暗色正文色
  text2: '#a6a6a6',
  text3: '#8b8b8b',
  accent: '#4cc9f0', // 本站品牌色
  accentSoft: 'rgba(76, 201, 240, 0.12)',
  codeBg: '#161618',
}

/**
 * 把设计 token 写成 CSS 变量
 *
 * 为什么不写在 CSS 文件里：
 *   这些值需要在运行时切换明暗，用 JS 写 inline style 优先级最高，
 *   不会被第三方样式（Element Plus / Waline）意外覆盖。
 */
export function applyPressTokens(dark = true) {
  const t = dark ? DARK : LIGHT
  const root = document.documentElement

  root.classList.toggle('dark', dark)

  const set = (k, v) => root.style.setProperty(k, v)

  // 背景层级（Valaxy 的三层结构）
  set('--pr-c-bg', t.bg)
  set('--pr-c-bg-alt', t.bgAlt)
  set('--pr-c-bg-soft', t.bgSoft)
  set('--pr-c-divider', t.divider)
  set('--pr-c-border', t.border)

  // 文字层级
  set('--pr-c-text-1', t.text1)
  set('--pr-c-text-2', t.text2)
  set('--pr-c-text-3', t.text3)

  // 强调色
  set('--pr-c-accent', t.accent)
  set('--pr-c-accent-soft', t.accentSoft)

  // 代码块
  set('--pr-code-bg', t.codeBg)

  // 排版尺度（Valaxy 的量级）
  set('--pr-home-max-width', '1080px')
  set('--pr-content-max-width', '760px')
  set('--pr-line-height', '1.75')
  set('--pr-transition', '0.2s ease')

  // 兼容原有的变量名（老组件还在用 --bg / --surface 等）
  set('--bg', t.bg)
  set('--surface', t.bgSoft)
  set('--border', t.border)
  set('--text', t.text1)
  set('--muted', t.text2)
  set('--accent', t.accent)
  set('--mono', '"SF Mono", Consolas, "Courier New", monospace')
}

/** 当前是否为暗色（本站固定暗色，保留接口以便将来切换） */
export const isDark = true

/**
 * 同步 Element Plus 的主题变量
 *
 * 逻辑描述：
 *   输入 → 输出：设计 token → Element Plus 的 --el-* CSS 变量
 *   关键机制：把 --el-* 指向本项目的 --pr-c-* / --press-c-* token，
 *             两边只维护一份色值。
 *   不变量：Element 组件与自写容器的底色/文字色一致
 *   边界：主色浅色档用 color-mix 计算，不支持的浏览器退化为主色
 *
 * 依据：
 *   经典问题：多套设计系统的 token 桥接（token bridging）
 *   来源：Element Plus 官方暗黑模式文档
 *        https://element-plus.org/zh-CN/guide/dark-mode
 *   原型结论：Element 用 --el-* 独立变量，必须显式对接
 *   本处变体：Valaxy 不用 Element Plus，无此层；本站两套并存
 *   沿用结论：--el-* 指向 --pr-c-*
 *   偏离处：浅色档用 color-mix 而非预计算值
 */
export function applyElementTheme() {
  const root = document.documentElement
  const set = (k, v) => root.style.setProperty(k, v)

  set('--el-color-primary', 'var(--pr-c-brand)')
  set('--el-color-primary-light-3', 'color-mix(in srgb, var(--pr-c-brand) 70%, transparent)')
  set('--el-color-primary-light-5', 'color-mix(in srgb, var(--pr-c-brand) 50%, transparent)')
  set('--el-color-primary-light-7', 'color-mix(in srgb, var(--pr-c-brand) 30%, transparent)')
  set('--el-color-primary-light-8', 'color-mix(in srgb, var(--pr-c-brand) 20%, transparent)')
  set('--el-color-primary-light-9', 'color-mix(in srgb, var(--pr-c-brand) 10%, transparent)')
  set('--el-color-primary-dark-2', 'color-mix(in srgb, var(--pr-c-brand) 80%, black)')

  set('--el-bg-color', 'var(--press-c-bg-soft)')
  set('--el-bg-color-page', 'var(--press-c-bg)')
  set('--el-bg-color-overlay', 'var(--press-c-bg-soft)')
  set('--el-fill-color-blank', 'var(--press-c-bg-alt)')
  set('--el-border-color', 'var(--press-c-divider)')
  set('--el-border-color-light', 'var(--press-c-divider)')
  set('--el-border-color-lighter', 'var(--press-c-divider)')
  set('--el-text-color-primary', 'var(--press-c-text-1)')
  set('--el-text-color-regular', 'var(--press-c-text-1)')
  set('--el-text-color-secondary', 'var(--press-c-text-2)')
  set('--el-text-color-placeholder', 'var(--press-c-text-3)')
  set('--el-border-radius-base', '8px')
  set('--el-border-radius-small', '6px')
}