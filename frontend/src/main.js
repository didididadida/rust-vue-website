import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import { warmupSanitizer } from './markdown'
import './style.css'
import './anime.css'

// 先预热 HTML 净化器再挂载：所有 v-html 渲染都依赖它，
// 若等首次渲染才懒加载，首屏会走「纯文本降级」分支，出现一次闪烁。
warmupSanitizer().finally(() => {
  const app = createApp(App)
  app.use(router)
  app.mount('#app')
})
