import axios from 'axios'
import { clearAuth } from './auth'

/**
 * 后端统一返回 `{ code, message, data }`。
 * 这里统一拆包：业务代码直接拿到 `data`，失败时抛出 ApiError（带可读 message）。
 */
export class ApiError extends Error {
  constructor(message, code) {
    super(message)
    this.name = 'ApiError'
    this.code = code
  }
}

const http = axios.create({ timeout: 30000 })

http.interceptors.request.use(config => {
  const token = localStorage.getItem('admin_token')
  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

http.interceptors.response.use(
  response => {
    const body = response.data
    if (body && typeof body === 'object' && 'code' in body) {
      if (body.code !== 0) {
        return Promise.reject(new ApiError(body.message || '请求失败', body.code))
      }
      return body.data
    }
    return body
  },
  error => {
    const status = error.response?.status
    const body = error.response?.data
    const message = body?.message || body?.detail || error.message || '网络请求失败'

    if (status === 401) {
      clearAuth()
      const { pathname, search } = window.location
      if (!pathname.startsWith('/login')) {
        window.location.assign(`/login?redirect=${encodeURIComponent(pathname + search)}`)
      }
    }
    return Promise.reject(new ApiError(message, status || body?.code))
  }
)

export default http
