<!--
  WalineComments —— 文章评论区（基于 @waline/client）

  逻辑描述：
    输入：文章 slug（用于构造 comment path）+ 可选配置覆盖
    输出：渲染一个完整的评论组件（评论列表 + 发表框 + 表情 + 楼层）
    关键机制：
      1. 用 @waline/client 的 init() 挂载到容器，path 决定评论归属哪篇文章；
      2. 组件卸载时必须调用 destroy()，否则切换文章会残留旧实例；
      3. 主题跟随站点暗色：直接注入 waline 的 CSS 变量覆盖；
      4. serverURL 指向 /comments（Nginx 反代到 waline-mini）
    不变量：
      - 同一时刻只有一个 Waline 实例挂在容器上；
      - path 变化时销毁旧实例再建新的。
    边界：
      - Waline 服务不可用时，组件显示加载失败提示，但**不影响文章正文阅读**；
      - 评论内容由 Waline 自己渲染（含 Markdown 与 XSS 过滤），
        不经过本站的 markdown.js，两者互不干扰。

  依据：
    经典问题：第三方组件的生命周期管理（mount / destroy 配对）
    来源：
      1. Waline 官方客户端文档
         https://waline.js.org/reference/client/
         要点：init() 返回带 destroy() 的对象；切换 path 需重建实例。
      2. Valaxy 官方评论插件（Valaxy 的评论实现就是 Waline）
         https://valaxy.site/zh/addons/official/waline
         配置形态：siteConfig.comment.enable + addonWaline({ serverURL })
      3. OWASP XSS 防护要求：第三方富文本必须由其自身净化
         https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html
    原型结论：
      1. 每次 init 都要有对应的 destroy，否则会出现重复实例与事件泄漏；
      2. 评论内容由 Waline 服务端净化后在客户端渲染（其内部用 DOMPurify 类方案）。
    本处变体：
      - Valaxy 用插件 + addon 的方式自动注册组件；
        本站是纯 Vue SPA，直接用 @waline/client 的 init() 更直接；
      - 需要适配站点暗色主题（Valaxy 的 Presse 主题有自己的一套色板）。
    沿用结论：保持 serverURL + path 两个核心参数；
              销毁逻辑放在 onBeforeUnmount 与 path 变化时。
    偏离处：
      - 未开启 pageview / reaction（本站有自己的浏览数统计），
        减少一次额外请求；
      - 未配置 OAuth 登录（只允许匿名昵称评论），
        降低配置复杂度；将来要开只需加 login 选项。
-->
<template>
  <section class="waline-section">
    <div class="waline-head">
      <h2 class="waline-title">
        评论
        <span v-if="loaded && count > 0" class="waline-count">{{ count }}</span>
      </h2>
    </div>

    <!-- 加载失败提示：不阻塞正文阅读 -->
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
import { onBeforeUnmount, onMounted, ref, watch, nextTick } from 'vue'

const props = defineProps({
  /** 文章 slug —— 决定评论归属哪一页 */
  slug: { type: String, required: true },
})

const host = ref(null)
const loaded = ref(false)
const failed = ref(false)
const count = ref(0)

/**
 * Waline 服务地址
 *
 * 用相对路径 /comments 而不是绝对 URL：
 *   - 同源，不触发 CORS；
 *   - 换域名时不用改代码；
 *   - Nginx 里 /comments/ 已反代到 waline-mini（127.0.0.1:8360）。
 */
const SERVER_URL = '/comments'

/** 当前实例，用于销毁 */
let instance = null

/** 站点的暗色 CSS 变量，注入给 Waline 使外观与站点一致 */
const DARK_VARS = {
  '--waline-bg-color': '#111318',
  '--waline-bg-color-light': '#151821',
  '--waline-bg-color-hover': '#1a1d26',
  '--waline-bgcolor': '#111318',
  '--waline-bgcolor-light': '#151821',
  '--waline-bgcolor-hover': '#1a1d26',
  '--waline-color': '#e8ecf1',
  '--waline-theme-color': '#4cc9f0',
  '--waline-active-color': '#6fd7f5',
  '--waline-border-color': '#23262f',
  '--waline-disable-bgcolor': '#151821',
  '--waline-disable-color': '#8b93a1',
  '--waline-info-bgcolor': '#151821',
  '--waline-info-color': '#8b93a1',
  '--waline-light-grey': '#23262f',
  '--waline-dark-grey': '#8b93a1',
  '--waline-border': '1px solid #23262f',
  '--waline-border-radius': '10px',
}

/**
 * 挂载 Waline 实例
 *
 * 关键机制：
 *   1. @waline/client 是纯客户端库，需要动态 import（避免进首屏包）；
 *   2. path 用 /blog/<slug>，与迁移脚本写入的 url 一致；
 *      不一致会导致历史评论显示不出来。
 */
async function mountWaline() {
  if (!host.value) return
  failed.value = false

  try {
    const { init } = await import('@waline/client')
    await import('@waline/client/style')

    // 销毁旧实例（path 变化时）
    if (instance) {
      instance.destroy()
      instance = null
    }
    host.value.innerHTML = ''

    instance = init({
      el: host.value,
      serverURL: SERVER_URL,
      // 评论归属路径：必须与迁移数据里的 url 完全一致
      path: `/blog/${props.slug}`,
      lang: 'zh-CN',
      // 只开评论，不开浏览量/表情反应（站点自己统计浏览数）
      pageview: false,
      reaction: false,
      comment: true,
      dark: 'html.dark',
      // 必须带 sortBy：waline-mini 不带该参数会返回空列表
      search: false,
      imageUploader: false,
      requiredMeta: ['nick'],
      login: 'disable',
    })

    loaded.value = true

    // 查询评论数用于标题展示
    try {
      const r = await fetch(
        `${SERVER_URL}/api/comment?path=/blog/${encodeURIComponent(props.slug)}&lang=zh-CN&pageSize=1&page=1&sortBy=insertedAt_desc`
      )
      const j = await r.json()
      count.value = j?.data?.count ?? 0
    } catch {
      // 计数失败不影响评论显示
    }
  } catch (e) {
    console.error('Waline 加载失败:', e)
    failed.value = true
  }
}

onMounted(() => {
  nextTick(mountWaline)
})

// 切换文章时重建实例
watch(
  () => props.slug,
  () => {
    loaded.value = false
    count.value = 0
    nextTick(mountWaline)
  }
)

// 组件卸载：必须销毁，否则实例与事件监听会残留
onBeforeUnmount(() => {
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
  border-top: 1px solid var(--border, #23262f);
}

.waline-head {
  margin-bottom: 1.2rem;
}

.waline-title {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--text, #e8ecf1);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.waline-count {
  font-size: 0.8rem;
  font-weight: 500;
  padding: 0.1rem 0.5rem;
  border-radius: 999px;
  background: rgba(76, 201, 240, 0.14);
  color: var(--accent, #4cc9f0);
  font-family: var(--mono, monospace);
}

.waline-error {
  margin-bottom: 1rem;
}

/* Waline 挂载点：让它的内部样式接管 */
.waline-host {
  /* 用 CSS 变量覆盖 Waline 的配色，使其融入站点暗色底 */
  --waline-bg-color: #111318;
  --waline-bg-color-light: #151821;
  --waline-bg-color-hover: #1a1d26;
  --waline-color: #e8ecf1;
  --waline-theme-color: #4cc9f0;
  --waline-active-color: #6fd7f5;
  --waline-border-color: #23262f;
  --waline-disable-bgcolor: #151821;
  --waline-disable-color: #8b93a1;
  --waline-info-bgcolor: #151821;
  --waline-info-color: #8b93a1;
  --waline-light-grey: #23262f;
  --waline-dark-grey: #8b93a1;
  --waline-border: 1px solid #23262f;
  --waline-border-radius: 10px;
}

/* Waline 内部元素的字体与站点对齐 */
.waline-host :deep(.wl-panel) {
  border-radius: 12px;
}

.waline-host :deep(.wl-editor) {
  font-family: inherit;
  font-size: 0.9rem;
  line-height: 1.7;
}

.waline-host :deep(.wl-comment) {
  font-size: 0.9rem;
}

.waline-host :deep(.wl-content) {
  line-height: 1.75;
}

/* 修复暗色下可能出现的白底输入框 */
.waline-host :deep(textarea),
.waline-host :deep(input) {
  background-color: #0b0d12;
  color: #e8ecf1;
}
</style>
