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
 * 相对路径 /comments：同源、免 CORS、换域名不用改代码。
 * Nginx 中 /comments/ 反代到 waline-mini（127.0.0.1:8360）。
 */
const SERVER_URL = '/comments'

/** 评论归属路径：必须与迁移数据里的 url 一致（/blog/<slug>） */
function pathOf(slug) {
  return `/blog/${slug}`
}

/**
 * 表情 CDN
 *
 * 照抄上游 utils/getEmojis 的默认值：三套表情包。
 * 用 unpkg 的 @waline/emojis 包。
 */
const EMOJI_CDN = 'https://unpkg.com/'
const EMOJI_TYPES = ['bilibili', 'qq', 'weibo']

function getEmojis() {
  return EMOJI_TYPES.map((t) => `${EMOJI_CDN}@waline/emojis/${t}/`)
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
    // 评论与浏览量分别由 commentCount/pageviewCount 处理，这里只开评论
    comment: true,
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
