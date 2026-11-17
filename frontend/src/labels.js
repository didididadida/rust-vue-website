/**
 * 全站共享的展示常量与小工具
 */

export const ARTICLE_CATEGORIES = [
  { label: '全部', value: 'all' },
  { label: '博客', value: 'blog' },
  { label: '随笔', value: 'essay' },
  { label: '技术', value: 'tech' }
]

const CATEGORY_LABELS = {
  blog: '博客',
  essay: '随笔',
  tech: '技术',
  dev: '开发工具',
  utility: '日常工具',
  productivity: '效率工具',
  other: '其他'
}

export const categoryLabel = cat => CATEGORY_LABELS[cat] || cat || '未分类'

export const formatDate = value => {
  if (!value) return ''
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric' })
}

export const formatDateShort = value => {
  if (!value) return ''
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleDateString('zh-CN')
}

export const formatDateTime = value => {
  if (!value) return ''
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  })
}
