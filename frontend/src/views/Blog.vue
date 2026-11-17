<template>
  <div class="blog-page">
    <h1 class="page-title">博客</h1>
    <p class="page-subtitle">技术思考、项目记录与生活随笔</p>

    <!-- 分类筛选 -->
    <div class="filters">
      <el-radio-group v-model="currentCategory" @change="filterByCategory">
        <el-radio-button v-for="cat in categories" :key="cat.value" :value="cat.value">
          {{ cat.label }}
        </el-radio-button>
      </el-radio-group>
    </div>

    <!-- 文章列表 -->
    <div v-if="loading" class="loading">
      <el-skeleton :rows="4" animated />
    </div>
    <div v-else-if="articles.length === 0" class="empty">
      <el-empty description="该分类下暂无文章" />
    </div>
    <div v-else class="article-list">
      <article v-for="item in articles" :key="item.id" class="card article-item">
        <div v-if="item.cover_image" class="item-cover">
          <el-image :src="item.cover_image" :alt="item.title" fit="cover" lazy />
        </div>
        <div class="item-body">
          <div class="item-meta">
            <el-tag size="small" type="info" effect="plain">{{ categoryLabel(item.category) }}</el-tag>
            <span v-if="!item.is_published" class="dot">·</span>
            <span class="dot">·</span>
            <span class="date">{{ formatDate(item.created_at) }}</span>
          </div>
          <h2 class="item-title">
            <router-link :to="'/blog/' + encodeURIComponent(item.slug)">{{ item.title }}</router-link>
          </h2>
          <p class="item-summary">{{ item.summary || stripMarkdown(item.content).slice(0, 150) + '...' }}</p>
          <div v-if="item.tags && item.tags.length" class="item-tags">
            <el-tag v-for="tag in item.tags" :key="tag" size="small" effect="plain" class="tag">{{ tag }}</el-tag>
          </div>
          <router-link :to="'/blog/' + encodeURIComponent(item.slug)" class="read-more">阅读全文 →</router-link>
        </div>
      </article>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import http from '../api'
import { ARTICLE_CATEGORIES, categoryLabel, formatDate } from '../labels'

const articles = ref([])
const loading = ref(true)
const currentCategory = ref('all')

const categories = ARTICLE_CATEGORIES

// 纯文本摘要：去掉 Markdown 标记与代码块
const stripMarkdown = (text) =>
  String(text || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/[#>*_`~\-\[\]()!]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()

const fetchArticles = async (category) => {
  loading.value = true
  try {
    const params = category && category !== 'all' ? { category } : {}
    articles.value = await http.get('/api/articles', { params })
  } catch (e) {
    console.error('加载文章失败:', e)
  } finally {
    loading.value = false
  }
}

const filterByCategory = (cat) => {
  currentCategory.value = cat
  fetchArticles(cat)
}

onMounted(() => fetchArticles('all'))
</script>

<style scoped>
/* ---------- 分类筛选 ---------- */
.filters {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 2.25rem;
  flex-wrap: wrap;
}

.filter-btn {
  padding: 0.45rem 1.1rem;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-full);
  cursor: pointer;
  font-size: 0.85rem;
  font-family: inherit;
  letter-spacing: 0.02em;
  transition: color 0.2s var(--ease), border-color 0.2s var(--ease),
    background 0.2s var(--ease);
}

.filter-btn:hover {
  border-color: var(--border-strong);
  color: var(--text-primary);
  background: var(--bg-raised);
}

.filter-btn.active {
  background: var(--accent-dim);
  border-color: rgba(123, 108, 246, 0.4);
  color: var(--accent-soft);
  font-weight: 500;
}

/* ---------- 列表 ---------- */
.article-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.article-item {
  display: flex;
  gap: 1.75rem;
  padding: 0;
  overflow: hidden;
  align-items: stretch;
}

.item-cover {
  width: 220px;
  flex-shrink: 0;
  overflow: hidden;
  background: var(--bg-overlay);
}

.item-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.5s var(--ease);
}

.article-item:hover .item-cover img {
  transform: scale(1.05);
}

.item-body {
  padding: 1.5rem 1.6rem 1.5rem 0;
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.article-item:not(:has(.item-cover)) .item-body {
  padding-left: 1.6rem;
}

.item-meta {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  margin-bottom: 0.6rem;
  flex-wrap: wrap;
}

.category-badge {
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  color: var(--accent);
  background: var(--accent-dim);
  padding: 0.15rem 0.55rem;
  border-radius: var(--radius-full);
  border: 1px solid rgba(123, 108, 246, 0.2);
}

.dot {
  color: var(--text-muted);
  font-size: 0.75rem;
}

.date {
  font-size: 0.78rem;
  color: var(--text-muted);
}

.item-title {
  font-family: var(--font-serif);
  font-size: 1.25rem;
  font-weight: 600;
  line-height: 1.5;
  margin-bottom: 0.55rem;
}

.item-title a {
  color: var(--text-primary);
  transition: color 0.2s var(--ease);
}

.item-title a:hover {
  color: var(--accent-soft);
}

.item-summary {
  font-size: 0.9rem;
  color: var(--text-secondary);
  line-height: 1.75;
  margin-bottom: 0.85rem;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.item-tags {
  display: flex;
  flex-wrap: wrap;
  margin-bottom: 0.75rem;
}

.read-more {
  font-size: 0.85rem;
  color: var(--accent-soft);
  margin-top: auto;
  align-self: flex-start;
  transition: color 0.2s var(--ease);
}

.read-more::after {
  content: ' →';
  display: inline-block;
  transition: transform 0.2s var(--ease);
}

.read-more:hover::after {
  transform: translateX(3px);
}

/* ---------- 响应式 ---------- */
@media (max-width: 768px) {
  .article-item {
    flex-direction: column;
    gap: 0;
  }

  .item-cover {
    width: 100%;
    height: 170px;
  }

  .item-body,
  .article-item:not(:has(.item-cover)) .item-body {
    padding: 1.25rem;
  }

  .item-title {
    font-size: 1.1rem;
  }
}
</style>