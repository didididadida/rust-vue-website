import { reactive, computed } from 'vue'

/**
 * 登录状态（token 存 localStorage，刷新后保持）
 */
export const auth = reactive({
  token: localStorage.getItem('admin_token') || '',
  username: localStorage.getItem('user_name') || '',
  nickname: localStorage.getItem('user_nickname') || '',
  avatar: localStorage.getItem('user_avatar') || '',
  isAdmin: localStorage.getItem('user_role') === 'admin',
})

export const isLoggedIn = computed(() => !!auth.token)

/** 顶栏展示用：优先昵称，其次用户名 */
export const displayName = computed(() => auth.nickname || auth.username)

export function setAuth({ token, username, nickname, avatar, is_admin }) {
  if (!token) {
    clearAuth()
    return
  }
  auth.token = token
  auth.username = username || ''
  auth.nickname = nickname || ''
  auth.avatar = avatar || ''
  auth.isAdmin = !!is_admin
  localStorage.setItem('admin_token', token)
  localStorage.setItem('user_name', auth.username)
  localStorage.setItem('user_nickname', auth.nickname)
  localStorage.setItem('user_avatar', auth.avatar)
  localStorage.setItem('user_role', auth.isAdmin ? 'admin' : 'user')
}

/** 仅更新展示信息，不动 token（资料保存后调用） */
export function updateDisplay({ nickname, avatar }) {
  if (nickname !== undefined) {
    auth.nickname = nickname || ''
    localStorage.setItem('user_nickname', auth.nickname)
  }
  if (avatar !== undefined) {
    auth.avatar = avatar || ''
    localStorage.setItem('user_avatar', auth.avatar)
  }
}

export function clearAuth() {
  auth.token = ''
  auth.username = ''
  auth.nickname = ''
  auth.avatar = ''
  auth.isAdmin = false
  localStorage.removeItem('admin_token')
  localStorage.removeItem('user_name')
  localStorage.removeItem('user_nickname')
  localStorage.removeItem('user_avatar')
  localStorage.removeItem('user_role')
}
