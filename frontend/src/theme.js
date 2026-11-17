/**
 * Element Plus 主题配置
 *
 * 逻辑描述：
 *   输入 → 输出：品牌色与背景色 → 覆盖 Element Plus 的 CSS 变量
 *   关键机制：
 *     1. "暗黑模式"在 Element Plus 里是给 <html> 加 `dark` 类
 *        （官方约定：`import 'element-plus/theme-chalk/dark/css-vars.css'`），
 *        组件再把颜色从 CSS 变量取值；
 *     2. 品牌色通过覆盖 `--el-color-primary` 系列变量实现，
 *        不重写组件样式，避免升级时被官方样式覆盖；
 *     3. 色值与 style.css 里既有的 --bg / --accent 等保持一致，
 *        让自写容器与 Element 组件没有色差。
 *   不变量：
 *     - 所有页面用到的颜色都能在这里找到来源，不允许在 .vue 里硬编码色值；
 *     - 切到亮色时不报错（变量有回退）。
 *   边界：
 *     - Element Plus 只覆盖它自己的组件；自写布局容器仍走 style.css 的 CSS 变量；
 *     - 覆盖变量必须在 dark css-vars 之后加载，否则会被官方默认值盖掉。
 *
 * 依据：
 *   经典问题：设计 token 的单一来源（design token centralization）
 *   来源：Element Plus 官方「暗黑模式」文档
 *        https://element-plus.org/zh-CN/guide/dark-mode.html
 *        要点：
 *          - 引入 `element-plus/theme-chalk/dark/css-vars.css`；
 *          - 给 html 添加 `dark` 类即启用；
 *          - 通过覆盖 CSS 变量自定义主题色。
 *        真实项目用法参考 Gitee「admin-web」
 *        https://gitee.com/hachimei/admin-web
 *          —— 其 README 说明主题状态在 setting 里切换 light/dark，
 *             菜单底色用 base.css 的 --menu-bg 覆盖。
 *   原型结论：
 *     1. 暗色靠 html.dark 类 + 官方暗色变量文件，不是自己写一套；
 *     2. 主色定制走 CSS 变量覆盖，比改 SCSS 变量更适合运行时切换。
 *   本处变体：站点原本是纯手写暗色 CSS，没有 UI 库；
 *             现在需要让 Element 的暗色与自写容器共用同一组色值。
 *   沿用结论：引入官方 dark css-vars，然后在同一文件里覆盖品牌色变量。
 *   偏离处：未使用 SCSS 变量定制（需要 sass 依赖与额外构建配置）；
 *           CSS 变量覆盖对运行时切换更友好，本项目的主题是固定的暗色，够用。
 */

/** 品牌主色：与 style.css 的 --accent 一致 */
export const BRAND_PRIMARY = '#4cc9f0'
export const BRAND_SUCCESS = '#4ade80'
export const BRAND_WARNING = '#fbbf24'
export const BRAND_ERROR = '#f87171'

/** 暗色背景层级：与 style.css 的 --bg / --surface / --border 一致 */
export const DARK_BG = '#08090d'
export const DARK_SURFACE = '#111318'
export const DARK_BORDER = '#23262f'
export const DARK_INPUT = '#0b0d12'

/**
 * 生成品牌色 CSS 变量覆盖
 *
 * Element Plus 的主色不是单一变量，而是一组：
 *   --el-color-primary            主色
 *   --el-color-primary-light-3/5/7/8/9   与白色混合的浅色（hover、禁用等）
 *   --el-color-primary-dark-2     与黑色混合的深色（active）
 * 只覆盖主色会让浅色档位仍是蓝色系，与品牌色冲突。
 *
 * 混合公式：mix(brand, white, weight) 表示品牌色占比 weight
 *   light-3 → 品牌 70% / 白 30%
 *   light-5 → 品牌 50% / 白 50%
 *   light-7 → 品牌 30% / 白 70%
 *   light-8 → 品牌 20% / 白 80%
 *   light-9 → 品牌 10% / 白 90%
 *   dark-2  → 品牌 80% / 黑 20%
 */
function mixWithWhite(hex, weight) {
  const n = parseInt(hex.slice(1), 16)
  const r = (n >> 16) & 0xff
  const g = (n >> 8) & 0xff
  const b = n & 0xff
  const mix = (c) => Math.round(c * weight + 255 * (1 - weight))
  return `#${((mix(r) << 16) | (mix(g) << 8) | mix(b)).toString(16).padStart(6, '0')}`
}

function mixWithBlack(hex, weight) {
  const n = parseInt(hex.slice(1), 16)
  const r = (n >> 16) & 0xff
  const g = (n >> 8) & 0xff
  const b = n & 0xff
  const mix = (c) => Math.round(c * weight)
  return `#${((mix(r) << 16) | (mix(g) << 8) | mix(b)).toString(16).padStart(6, '0')}`
}

/**
 * 把主题色写成 CSS 变量，注入到 document.documentElement 的 inline style
 *
 * 为什么不写进 style.css：
 *   这些值由品牌色推导而来，写在 JS 里能保证「改一处，全站跟着变」；
 *   写死在 CSS 里的话，改主色要同步改七个十六进制值，容易漏。
 */
export function applyTheme(root = document.documentElement) {
  const style = root.style
  style.setProperty('--el-color-primary', BRAND_PRIMARY)
  style.setProperty('--el-color-primary-light-3', mixWithWhite(BRAND_PRIMARY, 0.7))
  style.setProperty('--el-color-primary-light-5', mixWithWhite(BRAND_PRIMARY, 0.5))
  style.setProperty('--el-color-primary-light-7', mixWithWhite(BRAND_PRIMARY, 0.3))
  style.setProperty('--el-color-primary-light-8', mixWithWhite(BRAND_PRIMARY, 0.2))
  style.setProperty('--el-color-primary-light-9', mixWithWhite(BRAND_PRIMARY, 0.1))
  style.setProperty('--el-color-primary-dark-2', mixWithBlack(BRAND_PRIMARY, 0.8))

  style.setProperty('--el-color-success', BRAND_SUCCESS)
  style.setProperty('--el-color-warning', BRAND_WARNING)
  style.setProperty('--el-color-error', BRAND_ERROR)

  // 背景与边框：让 Element 组件融入站点暗色底
  style.setProperty('--el-bg-color', DARK_SURFACE)
  style.setProperty('--el-bg-color-page', DARK_BG)
  style.setProperty('--el-bg-color-overlay', DARK_SURFACE)
  style.setProperty('--el-fill-color-blank', DARK_INPUT)
  style.setProperty('--el-border-color', DARK_BORDER)
  style.setProperty('--el-border-color-light', DARK_BORDER)
  style.setProperty('--el-border-color-lighter', DARK_BORDER)
  style.setProperty('--el-text-color-primary', '#e8ecf1')
  style.setProperty('--el-text-color-regular', '#e8ecf1')
  style.setProperty('--el-text-color-secondary', '#8b93a1')
  style.setProperty('--el-border-radius-base', '10px')
  style.setProperty('--el-border-radius-small', '7px')
}

/**
 * 启用 / 关闭暗色
 *
 * Element Plus 的暗色约定：<html class="dark">。
 * 站点是纯暗色设计，默认就是暗色。
 */
export function setDark(on = true, root = document.documentElement) {
  root.classList.toggle('dark', on)
}
