<!--
  ImageBed —— 图床（/images）

  逻辑描述：
    输入：GET /api/images → [{ filename, url, size_human, modified_at, uploader, can_delete }]
    输出：上传区 + 图片网格；每张图标注上传者，仅对自己上传的图（或站长）显示删除按钮
    关键机制：
      - 权限以后端返回的 can_delete 为准，前端只负责展示/隐藏按钮
      - 顶部可切换「全部 / 我的」，站长默认看全部
    不变量：页面上出现的删除按钮，后端一定允许；反之即使被绕过也只会 403
    边界：历史图片没有归属记录，显示"未知上传者"，普通用户看不到删除按钮

  依据：
    经典问题：权限判定的单一可信源（server-side authorization）
    本处变体：删除按钮的存在与否需要在列表里一次性算好，不能逐张问后端
    沿用结论：后端在列表接口里把 can_delete 一并算好，前端不做任何权限推断
    偏离处：为了让页面不出现"人人可删"的错觉，仍展示 uploader 文案，
             但 is_admin 的显示规则完全由后端决定
-->
<template>
  <div class="image-bed">
    <h1 class="page-title">🖼️ 图床</h1>
    <p class="page-subtitle">
      上传图片获取外链。你可以删除自己上传的图片{{ isAdmin ? '，站长可以删除所有图片' : '' }}。
    </p>

    <!-- 上传区域 -->
    <div
      class="upload-zone"
      :class="{ dragging: isDragging }"
      @dragover.prevent="isDragging = true"
      @dragleave="isDragging = false"
      @drop.prevent="handleDrop"
      @click="triggerUpload"
    >
      <div class="upload-icon">📤</div>
      <p v-if="!uploading">拖拽图片到此处，或点击上传</p>
      <p v-else>上传中...</p>
      <p class="upload-hint">支持 PNG / JPG / GIF / WebP / SVG，最大 50MB</p>
      <input
        ref="fileInput"
        type="file"
        accept="image/*"
        multiple
        style="display:none"
        @change="handleFileSelect"
      />
    </div>

    <!-- 上传进度 -->
    <div v-if="uploading" class="upload-progress">
      <div class="progress-bar">
        <div class="progress-fill" :style="{ width: uploadProgress + '%' }"></div>
      </div>
      <span>{{ uploadProgress }}%</span>
    </div>

    <!-- 最近上传 -->
    <div v-if="lastUploaded.length" class="recent-uploads">
      <h3>✅ 上传成功</h3>
      <div v-for="img in lastUploaded" :key="img.filename" class="recent-item">
        <div v-for="img in lastUploaded" :key="img.filename" class="thumb-wrap">
          <img :src="thumbOf(img)" class="recent-thumb" :alt="img.filename" />
          <span v-if="img.is_video" class="video-badge">▶</span>
        </div>
        <div class="recent-info">
          <code class="url-text">{{ getFullUrl(img.url) }}</code>
          <div class="recent-actions">
            <button class="btn-sm btn" @click="copyUrl(getFullUrl(img.url))">📋 复制链接</button>
            <button class="btn-sm btn" @click="copyMarkdown(getFullUrl(img.url), img.filename)">📝 复制 Markdown</button>
          </div>
        </div>
      </div>
    </div>

    <!-- 图片列表 -->
    <div class="image-list-section">
      <div class="section-header">
        <h2>已上传图片</h2>
        <div class="header-tools">
          <button
            type="button"
            :class="['filter-btn', { active: onlyMine }]"
            @click="toggleOnlyMine"
          >{{ onlyMine ? '只看我的' : '全部图片' }}</button>
          <button class="btn btn-outline" @click="loadImages" :disabled="loading">
            {{ loading ? '加载中...' : '🔄 刷新' }}
          </button>
        </div>
      </div>

      <div v-if="loading" class="loading">加载中...</div>
      <div v-else-if="visibleImages.length === 0" class="empty">
        <p>{{ onlyMine ? '你还没有上传过图片' : '还没有上传过图片' }}</p>
      </div>
      <div v-else class="image-grid">
        <div v-for="img in visibleImages" :key="img.filename" class="image-card">
          <div class="image-preview" @click="previewImage = img">
            <img :src="thumbOf(img)" :alt="img.filename" loading="lazy" />
            <span v-if="img.is_video" class="video-badge">▶</span>
          </div>
          <div class="image-info">
            <div class="image-meta">
              <span class="image-owner" :class="{ unknown: !img.uploader }">
                {{ img.uploader ? '@' + img.uploader : '未知上传者' }}
              </span>
              <span class="image-size">{{ img.size_human }}</span>
            </div>
            <div class="image-actions">
              <button class="btn-sm btn" @click="copyUrl(getFullUrl(img.url))" title="复制链接">📋</button>
              <button class="btn-sm btn" @click="copyMarkdown(getFullUrl(img.url), img.filename)" title="复制 Markdown">📝</button>
              <button
                v-if="img.can_delete"
                class="btn-sm btn-outline"
                title="删除"
                @click="deleteImage(img)"
              >🗑️</button>
              <span v-else class="lock-hint" title="只能删除自己上传的图片">🔒</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 图片预览弹窗 -->
    <div v-if="previewImage" class="modal-overlay" @click.self="previewImage = null">
      <div class="preview-modal">
        <button class="close-btn" @click="previewImage = null">✕</button>
        <!-- 视频用原生播放器预览，图片用 img -->
        <video
          v-if="previewImage.is_video"
          :src="previewImage.transcoded_url || previewImage.url"
          :poster="previewImage.poster_url || undefined"
          class="preview-img"
          controls
          autoplay
        ></video>
        <img v-else :src="previewImage.url" class="preview-img" />
        <div class="preview-info">
          <code>{{ getFullUrl(previewImage.url) }}</code>
          <p class="preview-owner">
            上传者：{{ previewImage.uploader || '未知' }}
            <template v-if="previewImage.can_delete"> · 你可以删除这张图</template>
          </p>
          <div class="preview-actions">
            <button class="btn" @click="copyUrl(getFullUrl(previewImage.url))">📋 复制链接</button>
            <button class="btn" @click="copyMarkdown(getFullUrl(previewImage.url), previewImage.filename)">📝 复制 Markdown</button>
            <button
              v-if="previewImage.can_delete"
              class="btn btn-outline"
              @click="deleteImage(previewImage)"
            >🗑️ 删除</button>
          </div>
        </div>
      </div>
    </div>

    <!-- 提示消息 -->
    <div v-if="toast" class="toast">{{ toast }}</div>
  </div>
</template>

<script setup>
import { computed, ref, onMounted, onBeforeUnmount } from 'vue'
import http from '../api'
import { auth } from '../auth'

const fileInput = ref(null)
const images = ref([])
const loading = ref(false)
const uploading = ref(false)
const uploadProgress = ref(0)
const isDragging = ref(false)
const lastUploaded = ref([])
const previewImage = ref(null)
const toast = ref('')
const onlyMine = ref(false)

const isAdmin = computed(() => !!auth.isAdmin)

/** 「只看我的」用后端算好的 can_delete 不合适（站长全可删），这里按 uploader 过滤 */
const visibleImages = computed(() =>
  onlyMine.value
    ? images.value.filter(img => img.uploader && img.uploader.toLowerCase() === (auth.username || '').toLowerCase())
    : images.value
)

function toggleOnlyMine() {
  onlyMine.value = !onlyMine.value
}

function getFullUrl(url) {
  return window.location.origin + url
}

/**
 * 列表缩略图地址。
 *
 * 图片走 `/api/thumb/{name}?w=400`：后端按需生成 400 宽的 WebP 并缓存，
 * 列表一次铺几十个格子时不必各自加载原图（原图动辄 2MB+）。
 * 后端明确拒绝 SVG / ICO（无法当位图缩略），此时退回原图地址。
 * 视频用转码时生成的 `xxx.poster.jpg` 首帧封面——
 * 否则 <img> 加载 mp4 会得到一个空白/破图。没有封面时返回空串，
 * 由 CSS 的背景色 + ▶ 角标兜底。
 */
function thumbOf(img) {
  if (!img) return ''
  if (img.is_video) return img.poster_url || ''
  return thumbUrl(img.filename)
}

/** 位图 → 缩略图接口；其它格式（svg/ico）直接用原地址 */
function thumbUrl(filename) {
  const ext = String(filename || '').split('.').pop().toLowerCase()
  if (['png', 'jpg', 'jpeg', 'webp'].includes(ext)) {
    return `/api/thumb/${encodeURIComponent(filename)}?w=400`
  }
  return `/static/uploads/${encodeURIComponent(filename)}`
}

function showToast(msg) {
  toast.value = msg
  setTimeout(() => { toast.value = '' }, 2000)
}

async function copyUrl(url) {
  try {
    await navigator.clipboard.writeText(url)
    showToast('链接已复制！')
  } catch {
    showToast('复制失败，请手动复制')
  }
}

async function copyMarkdown(url, name) {
  const md = `![${name}](${url})`
  try {
    await navigator.clipboard.writeText(md)
    showToast('Markdown 已复制！')
  } catch {
    showToast('复制失败')
  }
}

function triggerUpload() {
  fileInput.value?.click()
}

async function uploadFiles(files) {
  uploading.value = true
  lastUploaded.value = []
  const total = files.length
  let done = 0

  for (const file of files) {
    if (!file.type.startsWith('image/')) continue
    const formData = new FormData()
    formData.append('file', file)
    try {
      const uploaded = await http.post('/api/upload', formData, {
        headers: { 'Content-Type': 'multipart/form-data' },
        onUploadProgress: (e) => {
          if (e.total) {
            uploadProgress.value = Math.round((done + e.loaded / e.total) / total * 100)
          }
        }
      })
      lastUploaded.value.push(uploaded)
    } catch (e) {
      showToast(`上传失败: ${file.name}（${e.message}）`)
    }
    done++
  }

  uploading.value = false
  uploadProgress.value = 0
  if (lastUploaded.value.length) {
    await loadImages()
  }
}

function handleFileSelect(e) {
  if (e.target.files.length) {
    uploadFiles(e.target.files)
    e.target.value = ''
  }
}

function handleDrop(e) {
  isDragging.value = false
  if (e.dataTransfer.files.length) {
    uploadFiles(e.dataTransfer.files)
  }
}

async function loadImages() {
  loading.value = true
  try {
    images.value = await http.get('/api/images')
  } catch (e) {
    console.error('加载图片列表失败:', e)
    showToast('加载失败：' + e.message)
  } finally {
    loading.value = false
  }
}

/**
 * 删除图片：权限由后端判定（can_delete 只在后端为真时才渲染按钮）。
 * 这里不用 confirm 的默认文案，而是明确说清删的是什么、删了会怎样。
 */
async function deleteImage(img) {
  const owner = img.uploader ? `（@${img.uploader} 上传）` : '（无归属记录）'
  if (!confirm(`确定删除这张图片${owner}？\n引用了它的文章/帖子会变成裂图。`)) return
  try {
    await http.delete(`/api/images/${encodeURIComponent(img.filename)}`)
    showToast('已删除')
    previewImage.value = null
    await loadImages()
  } catch (e) {
    showToast('删除失败：' + e.message)
  }
}

// 支持粘贴上传
function handlePaste(e) {
  const items = e.clipboardData?.items
  if (!items) return
  const files = []
  for (const item of items) {
    if (item.type.startsWith('image/')) {
      const f = item.getAsFile()
      if (f) files.push(f)
    }
  }
  if (files.length) {
    uploadFiles(files)
  }
}

onMounted(() => {
  loadImages()
  document.addEventListener('paste', handlePaste)
})

onBeforeUnmount(() => {
  document.removeEventListener('paste', handlePaste)
})
</script>

<style scoped>
.upload-zone {
  border: 2px dashed var(--border);
  border-radius: 12px;
  padding: 40px;
  text-align: center;
  cursor: pointer;
  transition: all 0.2s;
  margin-bottom: 20px;
}

.upload-zone:hover,
.upload-zone.dragging {
  border-color: var(--accent);
  background: rgba(76, 201, 240, 0.05);
}

.upload-icon {
  font-size: 2.5rem;
  margin-bottom: 10px;
}

.upload-hint {
  font-size: 0.8rem;
  color: var(--text-secondary);
  margin-top: 8px;
}

.upload-progress {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
}

.progress-bar {
  flex: 1;
  height: 6px;
  background: var(--border);
  border-radius: 3px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width 0.3s;
}

.recent-uploads {
  background: rgba(46, 213, 115, 0.1);
  border: 1px solid var(--success);
  border-radius: 8px;
  padding: 16px;
  margin-bottom: 24px;
}

.recent-uploads h3 {
  color: var(--success);
  margin-bottom: 12px;
}

.recent-item {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-bottom: 10px;
}

.recent-thumb {
  width: 60px;
  height: 60px;
  object-fit: cover;
  border-radius: 6px;
}

.recent-info {
  flex: 1;
}

.url-text {
  display: block;
  font-size: 0.8rem;
  color: var(--text-secondary);
  word-break: break-all;
  margin-bottom: 6px;
  background: var(--bg-secondary);
  padding: 4px 8px;
  border-radius: 4px;
}

.recent-actions {
  display: flex;
  gap: 6px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  gap: 12px;
  flex-wrap: wrap;
}

.section-header h2 {
  font-size: 1.3rem;
}

.header-tools {
  display: flex;
  gap: 8px;
  align-items: center;
}

.filter-btn {
  padding: 6px 16px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-secondary);
  border-radius: 20px;
  cursor: pointer;
  font-size: 0.84rem;
  font-family: inherit;
  transition: all 0.2s;
}

.filter-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.filter-btn.active {
  background: rgba(76, 201, 240, 0.16);
  border-color: rgba(76, 201, 240, 0.42);
  color: var(--accent-soft);
}

.image-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  margin-bottom: 6px;
}

.image-owner {
  font-size: 0.72rem;
  font-family: var(--font-mono);
  color: var(--accent-soft);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 历史图片没有归属记录：灰掉，避免被当成"自己的图"误点 */
.image-owner.unknown {
  color: var(--text-muted);
  font-family: inherit;
}

.lock-hint {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 4px 8px;
  font-size: 0.8rem;
  opacity: 0.45;
  cursor: not-allowed;
}

.image-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
}

.image-card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
  transition: transform 0.2s;
}

.image-card:hover {
  transform: translateY(-2px);
}

/* 视频角标容器：作为角标的定位基准 */
.thumb-wrap {
  position: relative;
  display: flex;
  flex-shrink: 0;
}

.video-badge {
  position: absolute;
  right: 6px;
  bottom: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.62);
  border: 1px solid rgba(255, 255, 255, 0.55);
  color: #fff;
  font-size: 0.62rem;
  line-height: 1;
  pointer-events: none;
}

.video-badge.big {
  width: 34px;
  height: 34px;
  font-size: 0.85rem;
}

/* 封面缺失时（转码还没跑完）用深色底兜底，而不是一片白 */
.image-preview:has(.video-badge) {
  background: linear-gradient(135deg, #1b2436, #10161f);
}
.image-preview {
  width: 100%;
  height: 140px;
  overflow: hidden;
  cursor: pointer;
  background: var(--bg-secondary);
  /* 视频角标定位基准 */
  position: relative;
}

.image-preview img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.image-info {
  padding: 8px 10px;
}

.image-size {
  font-size: 0.75rem;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.image-actions {
  display: flex;
  gap: 4px;
  align-items: center;
}

.btn-sm {
  padding: 4px 10px;
  font-size: 0.8rem;
}

/* 预览弹窗 */
.modal-overlay {
  position: fixed;
  top: 0; left: 0; right: 0; bottom: 0;
  background: rgba(0,0,0,0.85);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 20px;
}

.preview-modal {
  max-width: 90vw;
  max-height: 90vh;
  position: relative;
}

.close-btn {
  position: absolute;
  top: -40px;
  right: 0;
  background: none;
  border: none;
  color: #fff;
  font-size: 1.5rem;
  cursor: pointer;
}

.preview-img {
  max-width: 100%;
  max-height: 70vh;
  border-radius: 8px;
}

.preview-info {
  margin-top: 12px;
  text-align: center;
}

.preview-info code {
  display: block;
  font-size: 0.8rem;
  color: #aaa;
  word-break: break-all;
  margin-bottom: 10px;
}

.preview-owner {
  font-size: 0.8rem;
  color: var(--text-secondary);
  margin-bottom: 10px;
}

.preview-actions {
  display: flex;
  gap: 10px;
  justify-content: center;
}

/* Toast */
.toast {
  position: fixed;
  bottom: 30px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--success);
  color: #fff;
  padding: 10px 24px;
  border-radius: 8px;
  font-size: 0.9rem;
  z-index: 2000;
  animation: fadeIn 0.3s;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateX(-50%) translateY(10px); }
  to { opacity: 1; transform: translateX(-50%) translateY(0); }
}
</style>
