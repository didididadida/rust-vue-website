<template>
  <div class="login-page anime-page" data-bg="community">
    <div class="panel">
      <div class="panel-head">
        <div class="mark">飼</div>
        <p class="panel-eyebrow">STATION ACCESS</p>
        <h1 class="panel-title">wanle</h1>
        <p class="panel-sub">{{ isAdminEntry ? '管理控制台' : '登录你的账号' }}</p>
      </div>

      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-position="top"
        class="form"
        @submit.prevent="handleLogin"
      >
        <el-form-item label="用户名" prop="username">
          <el-input
            v-model.trim="form.username"
            placeholder="请输入用户名"
            autocomplete="username"
            :disabled="loading"
            size="large"
          />
        </el-form-item>

        <el-form-item label="密码" prop="password">
          <el-input
            v-model="form.password"
            type="password"
            placeholder="请输入密码"
            autocomplete="current-password"
            :disabled="loading"
            size="large"
            show-password
            @keyup.enter="handleLogin"
          />
        </el-form-item>

        <el-alert
          v-if="authError"
          :title="authError"
          type="error"
          :closable="false"
          show-icon
          class="form-error"
        />

        <el-button
          type="primary"
          size="large"
          class="btn-block"
          :loading="loading"
          native-type="submit"
        >
          {{ loading ? '登录中…' : '登录' }}
        </el-button>
      </el-form>

      <div class="panel-foot">
        <span>还没有账号？<router-link to="/register">注册成为船长</router-link></span>
        <router-link to="/" class="back">← 返回首页</router-link>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import http from '../api'
import { setAuth } from '../auth'

const route = useRoute()
const router = useRouter()

const isAdminEntry = computed(() => route.query.admin === '1')

// 不再预填 admin：预填会让用户误以为只需输密码
// 表单数据：用 reactive 对象承载，el-form 的 model 需要是对象
const form = reactive({ username: '', password: '' })
const formRef = ref(null)
const loading = ref(false)
const authError = ref('')

// 前端校验规则：只为提升体验，真正的校验在后端
const rules = {
  username: [{ required: true, message: '请输入用户名', trigger: 'blur' }],
  password: [{ required: true, message: '请输入密码', trigger: 'blur' }]
}

async function handleLogin() {
  authError.value = ''

  // 先走表单校验，未通过则不请求
  const valid = await formRef.value?.validate().catch(() => false)
  if (!valid) return

  loading.value = true
  try {
    const data = await http.post('/api/login', {
      username: form.username,
      password: form.password
    })
    setAuth(data)

    const redirect = route.query.redirect
    router.push(redirect || (data.is_admin ? '/admin' : '/me'))
  } catch (e) {
    authError.value = e.message || '登录失败'
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.login-page {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 68vh;
  padding: 1.5rem 0;
}

.panel {
  position: relative;
  width: 100%;
  max-width: 420px;
  background: rgba(16, 20, 32, 0.46);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 18px;
  padding: 2.4rem 2rem 1.9rem;
  box-shadow: 0 14px 46px rgba(0, 0, 0, 0.36);
  overflow: hidden;
}

/* 顶部冷光在二次元页面里换成柔和的粉白 */
.panel::before {
  content: '';
  position: absolute;
  top: 0;
  left: 25%;
  right: 25%;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(255, 210, 235, 0.8), transparent);
  opacity: 0.8;
}

.panel-head {
  text-align: center;
  margin-bottom: 1.9rem;
}

/* 站标：冷光方块 */
.mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 46px;
  margin-bottom: 1rem;
  border-radius: 12px;
  background: radial-gradient(circle at 32% 28%, #b6f0ff, #2b7fd4 55%, #4a3aa8);
  color: #03050b;
  font-family: var(--font-serif);
  font-size: 1.4rem;
  font-weight: 600;
  box-shadow: 0 0 24px rgba(76, 201, 240, 0.4),
    inset 0 0 12px rgba(255, 255, 255, 0.35);
}

.panel-eyebrow {
  font-family: var(--font-mono);
  font-size: 0.6rem;
  letter-spacing: 0.32em;
  color: var(--accent);
  opacity: 0.8;
  margin-bottom: 0.55rem;
}

.panel-title {
  font-family: var(--font-serif);
  font-size: 1.5rem;
  font-weight: 600;
  letter-spacing: 0.1em;
  color: #eef4ff;
  margin-bottom: 0.4rem;
}

.panel-sub {
  font-size: 0.83rem;
  color: var(--text-muted);
  letter-spacing: 0.03em;
}

/* ---------- 表单 ---------- */
.form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.field label {
  display: block;
  font-size: 0.8rem;
  color: var(--text-secondary);
  margin-bottom: 0.4rem;
  letter-spacing: 0.03em;
}

.input {
  width: 100%;
  padding: 0.72rem 0.9rem;
  background: #060a14;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: 0.95rem;
  font-family: inherit;
  transition: border-color 0.22s var(--ease), box-shadow 0.22s var(--ease);
}

.input::placeholder {
  color: var(--text-muted);
  opacity: 0.75;
}

.input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(76, 201, 240, 0.1);
}

.msg.error {
  font-size: 0.8rem;
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.08);
  border: 1px solid rgba(248, 113, 113, 0.22);
  padding: 0.45rem 0.65rem;
  border-radius: var(--radius-sm);
}

.btn-block {
  width: 100%;
  margin-top: 0.3rem;
}

/* ---------- 底部 ---------- */
.panel-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
  margin-top: 1.5rem;
  padding-top: 1.25rem;
  border-top: 1px solid var(--border);
  font-size: 0.8rem;
  color: var(--text-muted);
}

.panel-foot a {
  color: var(--accent);
}

.back {
  color: var(--text-muted) !important;
}

.back:hover {
  color: var(--text-secondary) !important;
}

@media (max-width: 480px) {
  .panel {
    padding: 1.9rem 1.25rem 1.5rem;
  }

  .panel-foot {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
