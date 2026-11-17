import { createRouter, createWebHistory } from 'vue-router'
import Home from '../views/Home.vue'
import Blog from '../views/Blog.vue'
import BlogDetail from '../views/BlogDetail.vue'
import Admin from '../views/Admin.vue'
import Login from '../views/Login.vue'
import Register from '../views/Register.vue'
import Profile from '../views/Profile.vue'
import ImageBed from '../views/ImageBed.vue'
import NotFound from '../views/NotFound.vue'
import { auth } from '../auth'

// 入口划分：
//   /        主页 —— 文章列表
//   /blog    博客列表与详情
//   /admin   管理后台
//   /images  图床（管理员）
const routes = [
  { path: '/', name: 'Home', component: Home },
  { path: '/blog', name: 'Blog', component: Blog },
  { path: '/blog/:slug', name: 'BlogDetail', component: BlogDetail },

  // 账号
  { path: '/login', name: 'Login', component: Login },
  { path: '/register', name: 'Register', component: Register },
  { path: '/me', name: 'MyProfile', component: Profile, meta: { requiresAuth: true } },
  { path: '/u/:username', name: 'Profile', component: Profile },
  { path: '/admin', name: 'Admin', component: Admin, meta: { requiresAuth: true, role: 'admin' } },
  { path: '/images', name: 'ImageBed', component: ImageBed, meta: { requiresAuth: true, role: 'admin' } },
  { path: '/:pathMatch(.*)*', name: 'NotFound', component: NotFound },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

// 路由守卫：需要登录的页面跳登录页，需要管理员的页面限制角色
router.beforeEach(to => {
  if (to.meta.requiresAuth && !auth.token) {
    return { path: '/login', query: { redirect: to.fullPath } }
  }
  if (to.meta.role === 'admin' && !auth.isAdmin) {
    return { path: '/me' }
  }
  if ((to.path === '/login' || to.path === '/register') && auth.token) {
    return { path: auth.isAdmin ? '/admin' : '/me' }
  }
  return true
})

export default router
