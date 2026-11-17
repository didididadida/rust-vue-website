/**
 * 应用入口
 *
 * 逻辑描述：
 *   输入 → 输出：Vue 应用实例 → 挂载到 #app，全局可用**必要**的 Element Plus 组件
 *   关键机制：
 *     1. 只 import 需要用的组件并逐个 `app.use()`，Vite 才能 tree shake；
 *     2. 主题在挂载前通过 applyTheme() 写到 <html> 的 inline style，
 *        优先级高于官方 dark css-vars，保证品牌色不被覆盖；
 *     3. 中文化：ConfigProvider + zhCn，避免分页/日期等组件显示英文。
 *   不变量：这里注册的组件名与模板里使用的 el-* 标签一一对应
 *   边界：漏注册的组件在模板里只是警告「Failed to resolve component」，
 *         **不会报错**，会渲染成空标签——改完必须实跑页面确认。
 *
 * 依据：
 *   经典问题：UI 库的引入方式与包体（bundle size）权衡
 *   来源：Element Plus 官方「快速开始」
 *        https://element-plus.org/zh-CN/guide/quickstart.html
 *          —— 完整引入 `app.use(ElementPlus)` 简单但打进全部组件；
 *             按需导入官方推荐 unplugin-vue-components + unplugin-auto-import。
 *        官方「暗黑模式」
 *        https://element-plus.org/zh-CN/guide/dark-mode
 *          —— 需要 dark/css-vars.css，并给 html 加 dark 类。
 *        第三方源码分析
 *        https://htui.tech/article/541
 *          —— 指出 dark css-vars 的颜色是静态写死的，
 *             直接覆盖 --el-* 要注意优先级与加载顺序。
 *   原型结论：
 *     1. 完整引入包体最大（本项目实测 1324 KB）；
 *     2. 逐个 import + app.use 也能被 tree shake，效果接近自动导入，且来源显式；
 *     3. 官方的 dark css-vars 是静态文件，品牌色需更高优先级覆盖。
 *   本处变体：9 个页面，共用组件稳定，不值得为自动导入再加两个构建插件。
 *   沿用结论：显式 import 需要的组件并逐个 use；主题用 inline style 注入。
 *   偏离处：没有接入 unplugin-auto-import。显式 import 让「组件从哪来」一目了然，
 *           出错信息也更准确——与本项目规模相称。
 */

import { createApp } from 'vue'

// —— 逐个导入用到的组件 ——
// 判据：出现在 3 个以上页面 → 全局注册；否则页面内局部 import。
import {
  // 基础
  ElButton,
  ElInput,
  ElTag,
  ElAvatar,
  ElEmpty,
  ElText,
  ElLink,
  ElDivider,
  ElIcon,
  // 布局
  ElRow,
  ElCol,
  ElSpace,
  ElContainer,
  ElHeader,
  ElMain,
  ElAside,
  // 数据展示
  ElTable,
  ElTableColumn,
  ElCard,
  ElSkeleton,
  ElSkeletonItem,
  ElImage,
  ElBadge,
  ElDescriptions,
  ElDescriptionsItem,
  // 反馈
  ElMessage,
  ElMessageBox,
  ElLoading,
  ElTooltip,
  ElPopover,
  ElPagination,
  // 导航
  ElDropdown,
  ElDropdownMenu,
  ElDropdownItem,
  ElMenu,
  ElMenuItem,
  // 表单
  ElForm,
  ElFormItem,
  ElSwitch,
  ElSelect,
  ElOption,
  ElCheckbox,
  ElRadioGroup,
  ElRadio,
  // 弹层
  ElDialog,
  ElDrawer,
  ElConfigProvider
} from 'element-plus'

import zhCn from 'element-plus/es/locale/lang/zh-cn'

// 样式顺序不能反：基础样式在前，暗色变量在后
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'

import App from './App.vue'
import router from './router'
import { warmupSanitizer } from './markdown'
import { applyTheme, setDark } from './theme'
import './style.css'
import './anime.css'

// 主题在创建应用前就位，避免首帧闪烁出默认蓝色
applyTheme()
setDark(true)

const components = [
  ElButton,
  ElInput,
  ElTag,
  ElAvatar,
  ElEmpty,
  ElText,
  ElLink,
  ElDivider,
  ElIcon,
  ElRow,
  ElCol,
  ElSpace,
  ElContainer,
  ElHeader,
  ElMain,
  ElAside,
  ElTable,
  ElTableColumn,
  ElCard,
  ElSkeleton,
  ElSkeletonItem,
  ElImage,
  ElBadge,
  ElDescriptions,
  ElDescriptionsItem,
  ElTooltip,
  ElPopover,
  ElPagination,
  ElDropdown,
  ElDropdownMenu,
  ElDropdownItem,
  ElMenu,
  ElMenuItem,
  ElForm,
  ElFormItem,
  ElSwitch,
  ElSelect,
  ElOption,
  ElCheckbox,
  ElRadioGroup,
  ElRadio,
  ElDialog,
  ElDrawer,
  ElConfigProvider
]

// 先预热 HTML 净化器再挂载：所有 v-html 渲染都依赖它，
// 若等首次渲染才懒加载，首屏会走「纯文本降级」分支，出现一次闪烁。
warmupSanitizer().finally(() => {
  const app = createApp(App)

  for (const c of components) {
    app.component(c.name, c)
  }

  // 指令与服务式组件（不是普通组件，不能 app.component 注册）
  app.use(ElLoading)

  // 让 useMessage()/useMessageBox() 等组合式 API 可用
  app.config.globalProperties.$message = ElMessage
  app.config.globalProperties.$msgbox = ElMessageBox

  app.use(router)
  app.mount('#app')
})
