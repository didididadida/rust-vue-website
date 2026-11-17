<!--
  WalineComments —— 文章评论区

  逻辑描述：
    输入：文章 slug（构造 comment path）
    输出：Waline 评论组件（列表 + 发表框 + 表情 + 楼层）+ 浏览量
    关键机制（对照 Valaxy 官方 valaxy-addon-waline 的实现）：
      1. 用 @waline/client 的 init() 挂载，path 决定归属；
      2. **watch + update()** 而不是 destroy 重建——切文章时不闪烁；
      3. **表情 CDN**：默认加载 bilibili / qq / weibo 三套（照抄上游默认值）；
      4. JS 与 CSS 并行加载后再 init，避免未样式化闪烁（FOUC）；
      5. 评论数用**列表接口**读 count，而非官方 commentCount()
         （waline-mini 未实现 type=count，详见 loadCounts 注释）。
    不变量：
      - 同一时刻只有一例；卸载时必须 destroy()；
      - path 变化时 update() 即可，不重建实例。
    边界：
      - Waline 服务不可用时显示提示，不阻塞正文；
      - 表情 CDN 用 unpkg，加载失败不影响评论本身。

  依据：
    经典问题：第三方评论组件的正确集成方式
    来源：
      Valaxy 官方 valaxy-addon-waline 的 WalineClient.vue
      https://github.com/YunYouJun/valaxy/blob/main/packages/valaxy-addon-waline/components/WalineClient.vue
      原文要点（逐条对照）：
        - `import { commentCount, init, pageviewCount } from '@waline/client'`
        - `walineStyleUrl from '@waline/client/waline.css?url'`（用 URL 而非直接 import）
        - onMounted 里先 ensureWalineStyle()，await 后再 init
        - `watch([...], () => walineInstance?.update(getWalineOptions()))`  ← 用 update 不重建
        - `onUnmounted(() => walineInstance?.destroy())`
        - `getEmojis(cdn, types)` 默认 ['bilibili', 'qq', 'weibo']
          https://github.com/YunYouJun/valaxy/blob/main/packages/valaxy-addon-waline/utils/index.ts
    原型结论：
      1. commentCount / pageviewCount 是独立函数，不要自己拼 REST 路径
         （我原先自己拼 type=count 少了 page 参数，导致一直失败）；
      2. 参数变化用 update() 比 destroy+init 平滑；
      3. 样式应等加载完成再 init，否则先渲染无样式内容再跳变。
    本处变体：
      - Valaxy 用 useRoute/useI18n/useAppStore 拿 path/locale/dark，
        本站是纯 Vue，直接传 props 与固定值；
      - Valaxy 的 ensureWalineStyle 处理 SSG 预加载与 CSP nonce，
        本站无 SSG，简化为「等 link.onload」。
    沿用结论：commentCount/pageviewCount 独立调用；update() 而非重建；
              表情用三套 CDN；样式就绪后再 init。
    偏离处：
      - 不支持明暗切换（本站固定暗色），故 watch 不含 isDark；
      - 未做 CSP nonce（本站 Nginx 的 CSP 不限制样式表来源）。
-->
<template>
  <section class="waline-section">
    <div class="waline-head">
      <h2 class="waline-title">
        评论
        <!-- 由 loadCounts() 填充 -->
        <span class="waline-count waline-comment-count"></span>
      </h2>
      <!-- 浏览量：waline-mini 无对外读接口，暂留空 -->
      <span class="waline-views waline-pageview-count"></span>
    </div>

    <el-alert
      v-if="failed"
      type="warning"
      :closable="false"
      show-icon
      title="评论服务暂时不可用"
      description="文章内容不受影响，稍后刷新可重试。"
      class="waline-error"
    />

    <div ref="host" class="waline-host"></div>
  </section>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps({
  /** 文章 slug —— 决定评论归属哪一页 */
  slug: { type: String, required: true },
})

const host = ref(null)
const failed = ref(false)

/**
 * Waline 服务地址
 *
 * **必须是绝对 URL，不能用相对路径。**
 *
 * 实测证据（在真实浏览器里对比三种写法）：
 *   serverURL = '/comments'
 *     字符串拼接     → '/comments/api/comment'      看起来对
 *     new URL(x)     → ERR: Invalid URL             ← 没有 base 就构造失败
 *     实际请求       → 'https:///comments/api/...'  ← 三个斜杠，必然失败
 *
 *   serverURL = location.origin + '/comments'
 *     字符串拼接     → 'https://sakesoul.top/comments/api/comment'
 *     new URL(x)     → 同样正确
 *
 * 为什么之前用相对路径不行：
 *   Waline 内部用 new URL(serverURL) 构造地址（无 base 参数），
 *   相对路径会抛 Invalid URL，最终降级成 'https:///...'，浏览器请求失败。
 *
 * 为什么要写成运行时计算而不是硬编码域名：
 *   硬编码 'https://sakesoul.top/comments' 会让本地开发（localhost:5173）
 *   也去打线上；用 location.origin 自动跟随当前域名。
 *
 * 依据：
 *   经典问题：相对 URL 的基准解析（relative URL resolution base）
 *   来源：WHATWG URL 标准 §4.4 basic URL parser
 *     https://url.spec.whatwg.org/#concept-basic-url-parser
 *     要点：相对 URL 必须提供 base；缺 base 时解析失败。
 *   原型结论：
 *     相对引用离开文档上下文就无法解析成绝对地址。
 *   本处变体：
 *     我的组件跑在浏览器里，本来有 location.origin 可用，
 *     但传进 Waline 的只是一个字符串，那个字符串离开了上下文。
 *   沿用结论：
 *     传出去之前先补全成绝对 URL。
 *   偏离处：
 *     用 location.origin 而非固定域名，保留多环境（本地/线上）能力。
 */
const SERVER_URL = `${window.location.origin}/comments`

/** 评论归属路径：必须与迁移数据里的 url 一致（/blog/<slug>） */
function pathOf(slug) {
  return `/blog/${slug}`
}

/**
 * 表情来源：**本地静态目录，不依赖外网 CDN**
 *
 * 为什么改成本地：
 *   实测同一台机器上
 *     命令行（走系统代理）  → unpkg.com 200
 *     浏览器（不走代理）    → Failed to fetch
 *   也就是说 unpkg.com 在这台机器的浏览器里根本连不上。
 *   表情是锦上添花的功能，不应该因为它拖垮整个评论区，
 *   更不应该让国内访客每次打开文章都发起三个注定失败的跨域请求。
 *
 * 仓库里已经带了 bilibili 全套表情图（94 个 png），
 * 放在 frontend/public/emojis/bilibili/，构建时原样发布到站点根。
 * 只补了一个 info.json（Waline 需要它来知道有哪些表情、用什么前缀）。
 *
 * 为什么不用 qq / weibo 两套：
 *   本地没有这两套图，硬配会 404。宁可少几套，不要带错误请求。
 *   以后要加，把图放进 public/emojis/<name>/ 并补 info.json 即可。
 *
 * 依据：
 *   经典问题：第三方依赖的可用性边界（external dependency availability）
 *   来源：本轮实测
 *     浏览器内 fetch('https://unpkg.com/@waline/emojis@1.2.0/bilibili/info.json')
 *       → {"err":"Failed to fetch"}
 *     命令行 Invoke-WebRequest 同一 URL → 200 application/json
 *   原型结论：
 *     命令行可达 ≠ 浏览器可达；两者的网络栈与代理设置不同。
 *   本处变体：
 *     个人站部署在国内，访客网络环境更接近「浏览器不可达 unpkg」。
 *   沿用结论：
 *     静态资源必须同源；跨域 CDN 只能作为可降级的增强。
 *   偏离处：
 *     表情数量少于官方三套，但换来零失败请求。
 *
 * 返回值的格式：目录路径（不带 info.json），由 Waline 自己补后缀。
 *   @waline/client@3.16.2 的 dist/waline.js 里有 `${e}/info.json`，
 *   我们多传一层文件名会变成 .../info.json/info.json → 404。
 */
const EMOJI_SOURCES = [`${window.location.origin}/emojis/bilibili/`]

function getEmojis() {
  return EMOJI_SOURCES
}
let instance = null
let destroyed = false

/**
 * 构造 init 参数
 *
 * 关键点：path 必须与历史数据的 url 完全一致，否则老评论不显示。
 */
function buildOptions() {
  return {
    serverURL: SERVER_URL,
    path: pathOf(props.slug),
    lang: 'zh-CN',
    dark: 'html.dark',
    emoji: getEmojis(),
    /**
     * 评论数统计 —— 关闭。
     *
     * Waline 的 `comment` 选项就是「评论数统计」（见 types 注释
     * "评论数统计 / Comment number support"，默认 true）。
     * 打开时它会请求 `?type=count`，而 waline-mini 没实现这个分支，
     * 返回 400 + 纯文本 "Query deserialize error: missing field `page`"，
     * 客户端按 JSON 解析就抛 "Unexpected token 'Q'"，污染控制台。
     *
     * 计数改由 loadCounts() 用列表接口的 count 字段取（返回结构里就有）。
     */
    comment: false,
    // waline-mini 均未实现的三项，关掉避免 400 请求。
    pageview: false,
    reaction: false,
    search: false,
    imageUploader: false,
    requiredMeta: ['nick'],
    login: 'disable',
  }
}


/** 挂载评论 */
async function mount() {
  if (!host.value || destroyed) return
  failed.value = false

  try {
    // 同时加载 JS 与 CSS：
    // Vite 会把 @waline/client/style 单独打成 chunk，与 JS 一起按需加载，
    // 这样首屏不加载这 22KB，且 init 时样式已就绪（无 FOUC）。
    const [{ init }] = await Promise.all([
      import('@waline/client'),
      import('@waline/client/style'),
    ])
    if (destroyed || !host.value) return

    instance = init({ el: host.value, ...buildOptions() })

    // 评论数：不用官方 commentCount()，因为它走 type=count 接口，
    // 而 waline-mini 未实现该参数（会返回 errno:1000 空数据）。
    // 改用列表接口读 count 字段，这是 waline-mini 能正常返回的路径。
    loadCounts()

    // 浏览量：waline-mini 的计数器（wl_counter 表）由服务端维护，
    // 同样通过列表接口拿不到，这里暂不显示（见下方说明）。
  } catch (e) {
    console.error('Waline 加载失败:', e)
    failed.value = true
  }
}


/**
 * 读取评论数
 *
 * 为什么不用官方 commentCount()：
 *   官方实现请求 `/api/comment?type=count&url=...`，
 *   但 waline-mini 未实现 type=count 分支（实测返回 errno:1000，无 data）。
 *   它的列表接口（带 sortBy）能正常返回 { count, data }，
 *   所以用 pageSize=1 的列表请求拿 count。
 *
 * 依据：
 *   经典问题：第三方实现的接口子集（partial API implementation）
 *   来源：waline-mini 实机测试
 *     - type=count + page  → errno:1000，无 data
 *     - 列表 + sortBy      → errno:0，data.count 正确
 *   原型结论：
 *     waline-mini 只实现了原版 Waline 的部分 API，
 *     集成时必须按实际可用的接口来，不能照客户端默认路径硬套。
 *   本处变体：
 *     上游 WalineClient.vue 用 commentCount() 是因为它跑在完整版 Waline 上。
 *   沿用结论：
 *     用列表接口读 count；pageSize=1 减少传输。
 *   偏离处：
 *     浏览量暂时不显示——waline-mini 的 wl_counter 表没有对外读取接口。
 */
async function loadCounts() {
  const url = `${SERVER_URL}/api/comment?path=${encodeURIComponent(pathOf(props.slug))}&lang=zh-CN&pageSize=1&page=1&sortBy=insertedAt_desc`
  try {
    const r = await fetch(url)
    const j = await r.json()
    const n = j?.data?.count
    if (typeof n === 'number') {
      const el = document.querySelector('.waline-comment-count')
      if (el) el.textContent = String(n)
    }
  } catch {
    // 计数失败不影响评论显示
  }
}

onMounted(mount)

/**
 * 切换文章：用 update() 而不是 destroy + init
 *
 * 参考上游：watch([options, path], () => instance?.update(...))
 * 这样不会整块重挂，列表平滑替换。
 */
watch(
  () => props.slug,
  (slug) => {
    if (instance) {
      instance.update(buildOptions())
    } else {
      // 首次挂载失败过，重试一次
      mount()
    }
  }
)

onBeforeUnmount(() => {
  destroyed = true
  if (instance) {
    instance.destroy()
    instance = null
  }
})
</script>

<style scoped>
.waline-section {
  margin-top: 3rem;
  padding-top: 2rem;
  border-top: 1px solid var(--press-c-divider-light);
}

.waline-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 1.2rem;
}

.waline-title {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--press-c-text-1);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.waline-count {
  font-size: 0.78rem;
  font-weight: 500;
  padding: 0.1rem 0.5rem;
  border-radius: 999px;
  background: var(--pr-c-brand-soft);
  color: var(--pr-c-brand);
  font-family: var(--pr-font-mono);
}

.waline-views {
  font-size: 0.76rem;
  color: var(--press-c-text-3);
  font-family: var(--pr-font-mono);
}

.waline-error {
  margin-bottom: 1rem;
}

/* 用站点 token 覆盖 Waline 变量，使配色一致 */
.waline-host {
  --waline-theme-color: var(--pr-c-brand);
  --waline-active-color: var(--pr-c-brand-hover);
  --waline-bg-color: var(--press-c-bg-soft);
  --waline-bg-color-light: var(--press-c-bg-alt);
  --waline-bg-color-hover: color-mix(in srgb, var(--press-c-bg-soft) 80%, white);
  --waline-color: var(--press-c-text-1);
  --waline-border-color: var(--press-c-divider);
  --waline-disable-bgcolor: var(--press-c-bg-alt);
  --waline-disable-color: var(--press-c-text-3);
  --waline-info-bgcolor: var(--press-c-bg-alt);
  --waline-info-color: var(--press-c-text-2);
  --waline-light-grey: var(--press-c-divider);
  --waline-dark-grey: var(--press-c-text-3);
  --waline-border-radius: 8px;
  --waline-border: 1px solid var(--press-c-divider);
}

.waline-host :deep(.wl-panel) {
  border-radius: 10px;
}

.waline-host :deep(.wl-editor),
.waline-host :deep(.wl-content) {
  font-family: var(--pr-font-body);
  line-height: 1.75;
}

.waline-host :deep(textarea),
.waline-host :deep(input) {
  background-color: var(--press-c-bg-alt);
  color: var(--press-c-text-1);
}
</style>
