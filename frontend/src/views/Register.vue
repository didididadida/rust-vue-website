<template>
  <div class="register-page anime-page" data-bg="community">
    <div class="panel">
      <div class="panel-head">
        <p class="panel-eyebrow">JOIN THE STATION</p>
        <h1 class="panel-title">登船</h1>
        <p class="panel-sub">注册后将成为本站的「船长」，拥有自己的主页</p>
      </div>

      <!-- 步骤指示器 -->
      <ol class="steps">
        <li :class="['step', { active: step >= 1, done: step > 1 }]">
          <span class="step-dot"></span>填写资料
        </li>
        <li :class="['step', { active: step >= 2 }]">
          <span class="step-dot"></span>验证邮箱
        </li>
      </ol>

      <!-- 第一步：用户名 / 邮箱 / 密码 -->
      <form v-if="step === 1" class="form" @submit.prevent="sendCode">
        <div class="field">
          <label for="reg-username">用户名</label>
          <input
            id="reg-username"
            v-model.trim="form.username"
            class="input"
            type="text"
            placeholder="2–20 个字符，可用中文"
            maxlength="20"
            autocomplete="username"
            :disabled="loading"
          />
        </div>

        <div class="field">
          <label for="reg-email">邮箱</label>
          <input
            id="reg-email"
            v-model.trim="form.email"
            class="input"
            type="email"
            placeholder="用于接收验证码"
            autocomplete="email"
            :disabled="loading"
          />
        </div>

        <div class="field">
          <label for="reg-password">密码</label>
          <input
            id="reg-password"
            v-model="form.password"
            class="input"
            type="password"
            placeholder="至少 6 个字符"
            autocomplete="new-password"
            :disabled="loading"
          />
        </div>

        <el-alert
          v-if="error"
          :title="error"
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
          {{ loading ? '发送中…' : '发送验证码' }}
        </el-button>
      </form>

      <!-- 第二步：验证码 -->
      <form v-else class="form" @submit.prevent="doRegister">
        <p class="sent-hint">
          验证码已发送至 <strong>{{ form.email }}</strong>
        </p>

        <div class="field">
          <label for="reg-code">6 位验证码</label>
          <input
            id="reg-code"
            v-model.trim="form.code"
            class="input code-input"
            type="text"
            inputmode="numeric"
            placeholder="000000"
            maxlength="6"
            autocomplete="one-time-code"
            :disabled="loading"
          />
        </div>

        <el-alert
          v-if="error"
          :title="error"
          type="error"
          :closable="false"
          show-icon
          class="form-error"
        />
        <el-alert
          v-else-if="notice"
          :title="notice"
          type="success"
          :closable="false"
          show-icon
          class="form-error"
        />

        <el-button
          type="primary"
          size="large"
          class="btn-block"
          :loading="loading"
          :disabled="form.code.length !== 6"
          native-type="submit"
        >
          {{ loading ? '验证中…' : '完成注册' }}
        </el-button>

        <div class="form-foot">
          <el-button link type="primary" :disabled="cooldown > 0" @click="resend">
            {{ cooldown > 0 ? `${cooldown} 秒后可重发` : '重新发送验证码' }}
          </el-button>
          <el-button link type="primary" @click="backToStep1">修改资料</el-button>
        </div>
      </form>

      <p class="panel-foot">
        已有账号？<router-link to="/login">去登录</router-link>
      </p>
    </div>
  </div>
</template>

<script setup>
import { reactive, ref, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import http from '../api'
import { setAuth } from '../auth'

const router = useRouter()

const step = ref(1)
const loading = ref(false)
const error = ref('')
const notice = ref('')
const cooldown = ref(0)

const form = reactive({ username: '', email: '', password: '', code: '' })

let timer = null
function startCooldown(seconds) {
  cooldown.value = seconds
  clearInterval(timer)
  timer = setInterval(() => {
    cooldown.value -= 1
    if (cooldown.value <= 0) clearInterval(timer)
  }, 1000)
}

onUnmounted(() => clearInterval(timer))

async function sendCode() {
  error.value = ''
  notice.value = ''
  if (!form.username || !form.email || !form.password) {
    error.value = '请填写完整的用户名、邮箱和密码'
    return
  }
  loading.value = true
  try {
    const data = await http.post('/api/register/send-code', {
      username: form.username,
      email: form.email,
      password: form.password
    })
    step.value = 2
    startCooldown(data?.resend_after ?? 60)
  } catch (e) {
    error.value = e.message || '发送失败'
  } finally {
    loading.value = false
  }
}

async function resend() {
  error.value = ''
  loading.value = true
  try {
    const data = await http.post('/api/register/send-code', {
      username: form.username,
      email: form.email,
      password: form.password
    })
    notice.value = '验证码已重新发送'
    form.code = ''
    startCooldown(data?.resend_after ?? 60)
  } catch (e) {
    error.value = e.message || '发送失败'
  } finally {
    loading.value = false
  }
}

async function doRegister() {
  error.value = ''
  loading.value = true
  try {
    const data = await http.post('/api/register', {
      email: form.email,
      code: form.code
    })
    setAuth(data)
    router.push('/me')
  } catch (e) {
    error.value = e.message || '注册失败'
  } finally {
    loading.value = false
  }
}

function backToStep1() {
  step.value = 1
  error.value = ''
  notice.value = ''
}
</script>

<style scoped>
.register-page {
  display: flex;
  justify-content: center;
  padding: 2rem 0 4rem;
}

.panel {
  width: 100%;
  max-width: 440px;
  background: rgba(16, 20, 32, 0.46);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 18px;
  padding: 2.25rem 2rem 2rem;
  position: relative;
  overflow: hidden;
  box-shadow: 0 14px 46px rgba(0, 0, 0, 0.36);
}

/* 顶部冷光 → 柔和粉白 */
.panel::before {
  content: '';
  position: absolute;
  top: 0;
  left: 20%;
  right: 20%;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(255, 210, 235, 0.8), transparent);
  opacity: 0.8;
}

.panel-head {
  text-align: center;
  margin-bottom: 1.75rem;
}

.panel-eyebrow {
  font-family: var(--font-mono);
  font-size: 0.62rem;
  letter-spacing: 0.32em;
  color: var(--accent);
  opacity: 0.85;
  margin-bottom: 0.7rem;
}

.panel-title {
  font-family: var(--font-serif);
  font-size: 1.75rem;
  font-weight: 600;
  letter-spacing: 0.12em;
  color: #eef4ff;
  margin-bottom: 0.5rem;
}

.panel-sub {
  font-size: 0.85rem;
  color: var(--text-muted);
  line-height: 1.7;
}

/* ---------- 步骤条 ---------- */
.steps {
  list-style: none;
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1.75rem;
}

.step {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 0.45rem;
  font-size: 0.78rem;
  color: var(--text-muted);
  padding: 0.5rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  transition: all 0.3s var(--ease);
}

.step.active {
  color: var(--accent);
  border-color: rgba(76, 201, 240, 0.35);
  background: rgba(76, 201, 240, 0.06);
}

.step.done {
  color: var(--success);
  border-color: rgba(52, 211, 153, 0.3);
}

.step-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: currentColor;
  flex-shrink: 0;
}

.step.active .step-dot {
  box-shadow: 0 0 8px currentColor;
}

/* ---------- 表单 ---------- */
.form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.field label {
  display: block;
  font-size: 0.82rem;
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

/* 验证码输入：等宽 + 大字距，像仪表读数 */
.code-input {
  font-family: var(--font-mono);
  font-size: 1.35rem;
  letter-spacing: 0.5em;
  text-align: center;
  padding-left: 0.5em;
}

.btn-block {
  width: 100%;
  margin-top: 0.35rem;
}

.msg {
  font-size: 0.82rem;
  line-height: 1.6;
  padding: 0.5rem 0.7rem;
  border-radius: var(--radius-sm);
}

.msg.error {
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.08);
  border: 1px solid rgba(248, 113, 113, 0.22);
}

.msg.ok {
  color: var(--success);
  background: rgba(52, 211, 153, 0.08);
  border: 1px solid rgba(52, 211, 153, 0.22);
}

.sent-hint {
  font-size: 0.85rem;
  color: var(--text-secondary);
  text-align: center;
  line-height: 1.7;
  padding: 0.6rem;
  background: rgba(76, 201, 240, 0.05);
  border: 1px dashed rgba(76, 201, 240, 0.2);
  border-radius: var(--radius-sm);
}

.sent-hint strong {
  color: var(--accent-soft);
  font-weight: 500;
  word-break: break-all;
}

.form-foot {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
  margin-top: 0.35rem;
}

.link {
  background: none;
  border: none;
  padding: 0;
  color: var(--text-muted);
  font-size: 0.8rem;
  font-family: inherit;
  cursor: pointer;
  transition: color 0.2s var(--ease);
}

.link:hover:not(:disabled) {
  color: var(--accent);
}

.link:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.panel-foot {
  text-align: center;
  font-size: 0.82rem;
  color: var(--text-muted);
  margin-top: 1.5rem;
  padding-top: 1.25rem;
  border-top: 1px solid var(--border);
}

.panel-foot a {
  color: var(--accent);
}

@media (max-width: 480px) {
  .panel {
    padding: 1.75rem 1.25rem 1.5rem;
  }
  .steps {
    flex-direction: column;
  }
}
</style>
