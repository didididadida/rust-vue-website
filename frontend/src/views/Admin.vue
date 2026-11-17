<template>
  <div class="admin-page">
    <h1 class="page-title">管理后台</h1>
    <p class="page-subtitle">
      所有内容都以 Markdown 文件保存：文章在 <code>posts/</code>，项目在 <code>data/toys/</code>，工具在 <code>data/tools/</code>，评论在 <code>data/comments/</code>
    </p>

    <!-- 标签切换 -->
    <div class="tabs">
      <button :class="['tab', { active: activeTab === 'articles' }]" @click="activeTab = 'articles'">📝 文章管理</button>
      <button :class="['tab', { active: activeTab === 'toys' }]" @click="activeTab = 'toys'">🚀 项目管理</button>
      <button :class="['tab', { active: activeTab === 'tools' }]" @click="activeTab = 'tools'">🔧 工具管理</button>
      <button :class="['tab', { active: activeTab === 'comments' }]" @click="activeTab = 'comments'">💬 评论管理</button>
    </div>

    <!-- 文章管理（DataTable：搜索 + 分页 + 多选 + 行内编辑） -->
    <div v-if="activeTab === 'articles'" class="tab-content">
      <DataTable
        v-model:page="articlePage"
        :rows="articlePageRows"
        :columns="articleColumns"
        :total="articleFiltered.length"
        :page-size="articlePageSize"
        :selectable="true"
        :row-id="row => row.slug"
        clickable-rows
        :muted-row="row => !row.is_published"
        searchable
        search-placeholder="搜索标题或别名…"
        empty-text="暂无文章"
        @search="k => (articleKeyword = k)"
        @selection-change="slugs => (articleSelection = slugs)"
        @row-click="row => openArticleEditor(row)"
      >
        <template #toolbar>
          <button class="btn" @click="openArticleEditor()">+ 新建文章</button>
          <button class="btn btn-outline" @click="loadArticles">刷新</button>
          <router-link to="/admin/trash" class="btn btn-outline">🗑️ 垃圾箱</router-link>
          <router-link to="/images" class="btn btn-outline">图床</router-link>
          <span v-if="loading.articles" class="loading-inline">加载中…</span>
        </template>

        <template #batch="{ selected }">
          <button type="button" class="dt-danger-btn" @click="batchDeleteArticles(selected)">
            删除选中（{{ selected.length }}）
          </button>
        </template>

        <!-- 标题：点整行直接打开编辑弹窗；草稿额外标出，避免和线上混为一谈 -->
        <template #cell-title="{ row }">
          <span class="cell-title" :class="{ 'is-draft': !row.is_published }">{{ row.title }}</span>
          <span v-if="!row.is_published" class="draft-tag" title="仅登录后可见，访客看不到">
            草稿 · 仅管理员可见
          </span>
        </template>

        <!-- 别名：就地编辑，失焦或回车保存 -->
        <template #cell-slug="{ row }">
          <input
            v-model.trim="row.slug"
            class="inline-input mono"
            :disabled="savingCell === row.slug"
            @blur="commitArticleField(row, 'slug')"
            @keyup.enter="$event.target.blur()"
            @keyup.esc="reloadArticlesQuiet"
          />
        </template>

        <!-- 分类：下拉即时保存 -->
        <template #cell-category="{ row }">
          <select
            v-model="row.category"
            class="inline-select"
            :disabled="savingCell === row.slug"
            @change="commitArticleField(row, 'category')"
          >
            <option v-for="c in articleCategoryOptions" :key="c.value" :value="c.value">
              {{ c.label }}
            </option>
          </select>
        </template>

        <!-- 状态：点击切换已发布 / 草稿 -->
        <template #cell-is_published="{ row }">
          <button
            type="button"
            :class="['status-btn', row.is_published ? 'published' : 'draft']"
            :disabled="savingCell === row.slug"
            :title="row.is_published ? '点击转为草稿' : '点击发布'"
            @click.stop="togglePublish(row)"
          >
            {{ row.is_published ? '已发布' : '草稿' }}
          </button>
        </template>
      </DataTable>
    </div>

    <!-- 项目管理（DataTable：搜索 + 分页 + 多选批量删除） -->
    <div v-if="activeTab === 'toys'" class="tab-content">
      <DataTable
        v-model:page="toyPage"
        :rows="toyPageRows"
        :columns="toyColumns"
        :total="toyFiltered.length"
        :page-size="pageSize"
        :selectable="true"
        :row-id="row => row.slug"
        :muted-row="row => !row.is_published"
        searchable
        search-placeholder="搜索名称或别名…"
        empty-text="暂无项目"
        @search="k => (toyKeyword = k)"
        @selection-change="slugs => (toySelection = slugs)"
      >
        <template #toolbar>
          <button class="btn" @click="openToyEditor()">+ 新建项目</button>
          <button class="btn btn-outline" @click="loadToys">刷新</button>
          <span v-if="loading.toys" class="loading-inline">加载中…</span>
        </template>

        <template #batch="{ selected }">
          <button type="button" class="dt-danger-btn" @click="batchDeleteToys(selected)">
            删除选中（{{ selected.length }}）
          </button>
        </template>

        <template #cell-name="{ row }">
          <span class="cell-title">{{ row.name }}</span>
        </template>

        <template #cell-slug="{ row }">
          <code>{{ row.slug }}</code>
        </template>

        <template #cell-tech_stack="{ row }">
          <span v-for="tech in row.tech_stack" :key="tech" class="tag">{{ tech }}</span>
        </template>

        <template #cell-is_published="{ row }">
          <span :class="['status', row.is_published ? 'published' : 'draft']">
            {{ row.is_published ? '已发布' : '草稿' }}
          </span>
        </template>

        <template #cell-actions="{ row }">
          <div class="actions">
            <button class="btn-sm btn" @click="openToyEditor(row)">编辑</button>
            <button class="btn-sm btn-outline" @click="deleteToy(row)">删除</button>
          </div>
        </template>
      </DataTable>
    </div>

    <!-- 工具管理（DataTable：搜索 + 分页 + 多选批量删除） -->
    <div v-if="activeTab === 'tools'" class="tab-content">
      <DataTable
        v-model:page="toolPage"
        :rows="toolPageRows"
        :columns="toolColumns"
        :total="toolFiltered.length"
        :page-size="pageSize"
        :selectable="true"
        :row-id="row => row.slug"
        :muted-row="row => !row.is_published"
        searchable
        search-placeholder="搜索名称或别名…"
        empty-text="暂无工具"
        @search="k => (toolKeyword = k)"
        @selection-change="slugs => (toolSelection = slugs)"
      >
        <template #toolbar>
          <button class="btn" @click="openToolEditor()">+ 新建工具</button>
          <button class="btn btn-outline" @click="loadTools">刷新</button>
          <span v-if="loading.tools" class="loading-inline">加载中…</span>
        </template>

        <template #batch="{ selected }">
          <button type="button" class="dt-danger-btn" @click="batchDeleteTools(selected)">
            删除选中（{{ selected.length }}）
          </button>
        </template>

        <template #cell-name="{ row }">
          <span class="cell-title">
            <span v-if="row.icon" class="cell-icon">{{ row.icon }}</span>{{ row.name }}
          </span>
        </template>

        <template #cell-slug="{ row }">
          <code>{{ row.slug }}</code>
        </template>

        <template #cell-category="{ row }">
          <span class="badge">{{ categoryLabel(row.category) }}</span>
        </template>

        <template #cell-is_published="{ row }">
          <span :class="['status', row.is_published ? 'published' : 'draft']">
            {{ row.is_published ? '已发布' : '草稿' }}
          </span>
        </template>

        <template #cell-actions="{ row }">
          <div class="actions">
            <button class="btn-sm btn" @click="openToolEditor(row)">编辑</button>
            <button class="btn-sm btn-outline" @click="deleteTool(row)">删除</button>
          </div>
        </template>
      </DataTable>
    </div>

    <!-- 评论管理（DataTable：搜索 + 分页 + 多选批量删除） -->
    <div v-if="activeTab === 'comments'" class="tab-content">
      <DataTable
        v-model:page="commentPage"
        :rows="commentPageRows"
        :columns="commentColumns"
        :total="commentFiltered.length"
        :page-size="pageSize"
        :selectable="true"
        :row-id="commentKey"
        searchable
        search-placeholder="搜索作者、内容或文章…"
        empty-text="暂无评论"
        @search="k => (commentKeyword = k)"
        @selection-change="keys => (commentSelection = keys)"
      >
        <template #toolbar>
          <button class="btn btn-outline" @click="loadComments">刷新</button>
          <label class="filter-toggle">
            <input v-model="onlyPending" type="checkbox" /> 只看待审核
          </label>
          <span v-if="loading.comments" class="loading-inline">加载中…</span>
        </template>

        <template #batch="{ selected }">
          <button type="button" class="dt-danger-btn" @click="batchDeleteComments(selected)">
            删除选中（{{ selected.length }}）
          </button>
        </template>

        <template #cell-slug="{ row }">
          <code>{{ row.slug }}</code>
        </template>

        <template #cell-author="{ row }">
          {{ row.author }}
        </template>

        <template #cell-content="{ row }">
          <span class="comment-cell">{{ row.content }}</span>
        </template>

        <template #cell-created_at="{ row }">
          {{ formatDateTime(row.created_at) }}
        </template>

        <template #cell-is_approved="{ row }">
          <span :class="['status', row.is_approved ? 'published' : 'draft']">
            {{ row.is_approved ? '已通过' : '待审核' }}
          </span>
        </template>

        <template #cell-actions="{ row }">
          <div class="actions">
            <button v-if="!row.is_approved" class="btn-sm btn" @click="approveComment(row, true)">通过</button>
            <button v-else class="btn-sm btn-outline" @click="approveComment(row, false)">隐藏</button>
            <button class="btn-sm btn-outline" @click="deleteComment(row)">删除</button>
          </div>
        </template>
      </DataTable>
    </div>


    <!-- 文章编辑器：全屏工作区（不是弹窗），左侧写正文、右侧放元数据 -->
    <div v-if="showArticleEditor" class="editor-page">
      <!-- 顶栏：标题 + 状态 + 操作，滚动时固定在顶部 -->
      <header class="editor-bar">
        <div class="editor-bar-left">
          <button type="button" class="bar-back" @click="closeArticleEditor" title="返回列表（Esc）">
            ← 返回
          </button>
          <span class="bar-title">
            {{ rawSlug ? '编辑文章' : '新建文章' }}
          </span>
          <span v-if="dirty" class="bar-dirty" title="有未保存的修改">● 未保存</span>
        </div>
        <div class="editor-bar-right">
          <button type="button" class="btn btn-outline" @click="closeArticleEditor">取消</button>
          <button type="button" class="btn" :disabled="saving" @click="saveRawArticle">
            {{ saving ? '保存中…' : '保存' }}
          </button>
        </div>
      </header>

      <!-- 主工作区：整篇 .md 就是一个大文本区，Markdown 与 front matter 都直接写 -->
      <div class="editor-body single">
        <main class="editor-main">
          <div class="raw-toolbar">
            <span class="raw-tab">
              <code>{{ rawSlug || '未命名' }}.md</code>
            </span>
            <span class="raw-sep">·</span>
            <button type="button" class="raw-btn" @click="insertSnippet('h2')" title="插入二级标题">## 小节</button>
            <button type="button" class="raw-btn" @click="insertSnippet('code')" title="插入代码块">```代码</button>
            <button type="button" class="raw-btn" @click="insertSnippet('quote')" title="插入引用">&gt; 引用</button>
            <button type="button" class="raw-btn" @click="insertSnippet('image')" title="插入图片语法">![图片]()</button>
            <span class="raw-tip">可直接粘贴截图上传</span>
          </div>

          <div class="field-relative content-wrap">
            <MarkdownEditor
              ref="rawEditorRef"
              v-model="rawText"
              height="calc(100vh - 300px)"
              placeholder="---&#10;title: 标题&#10;summary: 摘要&#10;category: blog&#10;tags:&#10;- 标签&#10;is_published: true&#10;---&#10;&#10;正文用 Markdown 写，可直接粘贴截图"
              @save="saveRawArticle"
            />
          </div>

          <div class="editor-status">
            <span>{{ rawLines }} 行</span>
            <span class="status-sep">·</span>
            <span>{{ rawText.length }} 字符</span>
            <span class="status-sep">·</span>
            <span>Ctrl+S 保存</span>
            <span v-if="rawError" class="status-error">⚠ {{ rawError }}</span>
          </div>
        </main>
      </div>
    </div>


    <!-- 项目编辑器：纯 Markdown（整篇 .md），与文章完全一致 -->
    <div v-if="showToyEditor" class="editor-page">
      <header class="editor-bar">
        <div class="editor-bar-left">
          <button type="button" class="bar-back" @click="closeToyEditor" title="返回列表">
            ← 返回
          </button>
          <span class="bar-title">{{ toySlug ? '编辑项目' : '新建项目' }}</span>
          <span v-if="toyDirty" class="bar-dirty" title="有未保存的修改">● 未保存</span>
        </div>
        <div class="editor-bar-right">
          <button type="button" class="btn btn-outline" @click="closeToyEditor">取消</button>
          <button type="button" class="btn" :disabled="saving" @click="saveToy">
            {{ saving ? '保存中…' : '保存' }}
          </button>
        </div>
      </header>

      <div class="editor-body single">
        <main class="editor-main">
          <div class="raw-toolbar">
            <span class="raw-tab">
              <code>data/toys/{{ toySlug || '未命名' }}.md</code>
            </span>
            <span class="raw-sep">·</span>
            <button type="button" class="raw-btn" @click="insertToySnippet('h2')" title="插入二级标题">## 小节</button>
            <button type="button" class="raw-btn" @click="insertToySnippet('code')" title="插入代码块">```代码</button>
            <button type="button" class="raw-btn" @click="insertToySnippet('quote')" title="插入引用">&gt; 引用</button>
            <button type="button" class="raw-btn" @click="insertToySnippet('image')" title="插入图片语法">![图片]()</button>
            <span class="raw-tip">可直接粘贴截图上传</span>
          </div>

          <div class="field-relative content-wrap">
            <MarkdownEditor
              ref="toyEditorRef"
              v-model="toyText"
              height="calc(100vh - 260px)"
              placeholder="---&#10;name: 项目名称&#10;description: 一句话介绍&#10;tech_stack:&#10;- Vue&#10;is_published: true&#10;---&#10;&#10;## 项目介绍"
              @save="saveToy"
            />
          </div>

          <div class="editor-status">
            <span>{{ toyText.split('\n').length }} 行</span>
            <span class="status-sep">·</span>
            <span>{{ toyText.length }} 字符</span>
            <span class="status-sep">·</span>
            <span>Ctrl+S 保存</span>
            <span v-if="toyError" class="status-error">⚠ {{ toyError }}</span>
          </div>
        </main>
      </div>
    </div>

    <!-- 工具编辑器：纯 Markdown（整篇 .md），与文章完全一致 -->
    <div v-if="showToolEditor" class="editor-page">
      <header class="editor-bar">
        <div class="editor-bar-left">
          <button type="button" class="bar-back" @click="closeToolEditor" title="返回列表">
            ← 返回
          </button>
          <span class="bar-title">{{ toolSlug ? '编辑工具' : '新建工具' }}</span>
          <span v-if="toolDirty" class="bar-dirty" title="有未保存的修改">● 未保存</span>
        </div>
        <div class="editor-bar-right">
          <button type="button" class="btn btn-outline" @click="closeToolEditor">取消</button>
          <button type="button" class="btn" :disabled="saving" @click="saveTool">
            {{ saving ? '保存中…' : '保存' }}
          </button>
        </div>
      </header>

      <div class="editor-body single">
        <main class="editor-main">
          <div class="raw-toolbar">
            <span class="raw-tab">
              <code>data/tools/{{ toolSlug || '未命名' }}.md</code>
            </span>
            <span class="raw-sep">·</span>
            <button type="button" class="raw-btn" @click="insertToolSnippet('h2')" title="插入二级标题">## 小节</button>
            <button type="button" class="raw-btn" @click="insertToolSnippet('code')" title="插入代码块">```代码</button>
            <button type="button" class="raw-btn" @click="insertToolSnippet('quote')" title="插入引用">&gt; 引用</button>
            <button type="button" class="raw-btn" @click="insertToolSnippet('image')" title="插入图片语法">![图片]()</button>
            <span class="raw-tip">可直接粘贴截图上传</span>
          </div>

          <div class="field-relative content-wrap">
            <MarkdownEditor
              ref="toolEditorRef"
              v-model="toolText"
              height="calc(100vh - 260px)"
              placeholder="---&#10;name: 工具名称&#10;description: 一句话介绍&#10;icon: 🔧&#10;category: utility&#10;url: https://...&#10;is_published: true&#10;---&#10;&#10;## 工具说明"
              @save="saveTool"
            />
          </div>

          <div class="editor-status">
            <span>{{ toolText.split('\n').length }} 行</span>
            <span class="status-sep">·</span>
            <span>{{ toolText.length }} 字符</span>
            <span class="status-sep">·</span>
            <span>Ctrl+S 保存</span>
            <span v-if="toolError" class="status-error">⚠ {{ toolError }}</span>
          </div>
        </main>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount, defineAsyncComponent } from 'vue'
import http from '../api'
import { formatDateTime, categoryLabel, ARTICLE_CATEGORIES } from '../labels'
import DataTable from '../components/DataTable.vue'
// 异步加载编辑器：CodeMirror + 语言包体积很大（约 600KB），
// 只在打开编辑页时才下载，不影响论坛/首页等普通页面
const MarkdownEditor = defineAsyncComponent(() => import('../components/MarkdownEditor.vue'))

const activeTab = ref('articles')
const saving = ref(false)
const loading = ref({ articles: false, toys: false, tools: false, comments: false })

const articles = ref([])

// ---------- 文章表格：搜索 / 分页 / 多选（状态由页面持有，DataTable 是受控组件）----------
const pageSize = 10
const articleKeyword = ref('')
const articlePage = ref(1)
const articlePageSize = pageSize
const articleSelection = ref([])

// ---------- 项目表格 ----------
const toyKeyword = ref('')
const toyPage = ref(1)
const toySelection = ref([])

// ---------- 工具表格 ----------
const toolKeyword = ref('')
const toolPage = ref(1)
const toolSelection = ref([])

// ---------- 评论表格 ----------
const commentKeyword = ref('')
const commentPage = ref(1)
const commentSelection = ref([])

// ---------- 文章行内编辑 ----------
//
// 设计：别名 / 分类 / 状态直接在表格里改，「编辑全文 / 删除」收进行的展开区。
// 这样常用字段改起来不用开弹窗，危险操作又不至于被误点到。
//
// 依据：
//   经典问题：就地编辑（in-place editing）与破坏性操作的隔离
//   本处变体：同一行既要支持"轻量字段随手改"，又要承载"整篇编辑/删除"
//   沿用结论：把低风险高频操作前置，高风险操作折叠进显式展开区
//   偏离处：原地编辑直接调用整篇 PUT（后端没有局部更新接口），
//           因此会把当前行的全部字段一起回写；字段来源是列表接口，非草稿态完整

/** 正在保存的行标识，用于禁用该行的编辑控件 */
const savingCell = ref('')

/** 分类下拉选项：「全部」是筛选用的伪分类，不能出现在编辑下拉里 */
const articleCategoryOptions = ARTICLE_CATEGORIES.filter(c => c.value !== 'all')

/** 静默重新拉取，用于放弃编辑时还原（Esc 键） */
async function reloadArticlesQuiet() {
  await loadArticles()
}

/**
 * 提交单个字段的修改。
 *
 * 输入 → 输出：行对象 + 字段名 → PUT /api/articles/{原 slug} → 重新加载列表
 * 关键机制：先记住原别名（row.slug 可能已被输入框改过），
 *           用原别名请求，payload 里带新别名，后端据此重命名文件
 * 不变量：改名后列表里展示的是新别名，且旧别名不再存在
 * 边界：别名为空时拒绝提交并回滚；分类/状态为枚举，无非法值
 *
 * 依据：
 *   经典问题：乐观更新与回滚（optimistic UI）
 *   本处变体：表格已就地显示新值，请求失败必须还原成服务端真实状态
 *   沿用结论：失败即重新拉取列表，用服务端数据覆盖本地乐观值
 *   偏离处：不做逐字段差异化 PUT，整行字段一起提交，避免定义新接口
 */
async function commitArticleField(row, field) {
  const originalSlug = row.originalSlug || row.slug

  if (field === 'slug' && !String(row.slug || '').trim()) {
    alert('别名不能为空')
    row.slug = originalSlug
    return
  }

  savingCell.value = originalSlug
  try {
    await http.put(`/api/articles/${path(originalSlug)}`, {
      title: row.title,
      slug: String(row.slug || '').trim(),
      summary: row.summary || '',
      category: row.category,
      tags: Array.isArray(row.tags) ? row.tags : [],
      cover_image: row.cover_image || '',
      content: row.content || '',
      is_published: !!row.is_published
    })
    await loadArticles()
  } catch (e) {
    alert('保存失败：' + e.message)
    await loadArticles()
  } finally {
    savingCell.value = ''
  }
}

/** 一键切换发布状态 */
async function togglePublish(row) {
  row.is_published = !row.is_published
  await commitArticleField(row, 'is_published')
}

const toys = ref([])
const tools = ref([])
const comments = ref([])
const onlyPending = ref(false)

// ---------- 文章表格：列 / 过滤 / 分页 ----------
//
// 这四个（articleColumns / articleFiltered / articlePageRows / batchDeleteArticles）
// 是模板直接引用的名字，缺任何一个都会让 /admin 在渲染时抛 TypeError 变白屏。
// 曾经在改行内编辑时被整段误删，导致管理页空白 —— 改动这块后请务必跑一次
// `npm run build` 并打开 /admin 确认有内容。
const articleColumns = [
  { key: 'title', label: '标题' },
  { key: 'slug', label: '别名', width: 170 },
  { key: 'category', label: '分类', width: 120 },
  { key: 'is_published', label: '状态', width: 100 },
  { key: 'created_at', label: '日期', width: 110 }
]

/**
 * 搜索过滤：标题或别名包含关键词（忽略大小写）。
 *
 * 依据：客户端过滤 vs 服务端过滤 —— 后台文章量在百级，
 *       一次拉 200 条后在前端筛选，切关键词零请求，体验更好。
 */
const articleFiltered = computed(() => {
  const k = articleKeyword.value.trim().toLowerCase()
  if (!k) return articles.value
  return articles.value.filter(
    a => (a.title || '').toLowerCase().includes(k) || (a.slug || '').toLowerCase().includes(k)
  )
})

/** 当前页要渲染的行 */
const articlePageRows = computed(() => {
  const start = (articlePage.value - 1) * articlePageSize
  return articleFiltered.value.slice(start, start + articlePageSize)
})

/**
 * 批量删除文章：逐条调用批量接口，返回里带上失败清单。
 * 依据：批量操作的部分失败 —— 删的是文件，没有事务可回滚，
 *       所以按"尽量做、逐条汇报"处理。
 */
async function batchDeleteArticles(slugs) {
  if (!slugs.length) return
  if (!confirm(`把选中的 ${slugs.length} 篇文章移入垃圾箱？可以在「垃圾箱」里恢复。`)) return
  try {
    const res = await http.post('/api/articles/batch-delete', { slugs })
    const okCount = (res.deleted || []).length
    const failed = res.failed || []
    if (failed.length) {
      alert(`已删除 ${okCount} 篇，失败 ${failed.length} 篇：\n` + failed.map(f => `${f.slug}：${f.error}`).join('\n'))
    }
    await loadArticles()
  } catch (e) {
    alert('批量删除失败：' + e.message)
  }
}

// ---------- 项目表格：列 / 过滤 / 分页 ----------
const toyColumns = [
  { key: 'name', label: '名称' },
  { key: 'slug', label: '别名', width: 150 },
  { key: 'tech_stack', label: '技术栈' },
  { key: 'is_published', label: '状态', width: 90 },
  { key: 'actions', label: '操作', width: 130 }
]

const toyFiltered = computed(() => {
  const k = toyKeyword.value.trim().toLowerCase()
  if (!k) return toys.value
  return toys.value.filter(
    t => (t.name || '').toLowerCase().includes(k) || (t.slug || '').toLowerCase().includes(k)
  )
})

const toyPageRows = computed(() => {
  const start = (toyPage.value - 1) * pageSize
  return toyFiltered.value.slice(start, start + pageSize)
})

// ---------- 工具表格：列 / 过滤 / 分页 ----------
const toolColumns = [
  { key: 'name', label: '名称' },
  { key: 'slug', label: '别名', width: 150 },
  { key: 'category', label: '分类', width: 110 },
  { key: 'is_published', label: '状态', width: 90 },
  { key: 'actions', label: '操作', width: 130 }
]

const toolFiltered = computed(() => {
  const k = toolKeyword.value.trim().toLowerCase()
  if (!k) return tools.value
  return tools.value.filter(
    t => (t.name || '').toLowerCase().includes(k) || (t.slug || '').toLowerCase().includes(k)
  )
})

const toolPageRows = computed(() => {
  const start = (toolPage.value - 1) * pageSize
  return toolFiltered.value.slice(start, start + pageSize)
})

// ---------- 评论表格：列 / 过滤 / 分页 ----------
//
// 评论是「文章下的从属实体」，没有全站唯一 id，
// 因此行标识统一用 slug/id 复合键，批量删除也按这个键回传。
const commentColumns = [
  { key: 'slug', label: '文章', width: 140 },
  { key: 'author', label: '昵称', width: 110 },
  { key: 'content', label: '内容' },
  { key: 'created_at', label: '时间', width: 150 },
  { key: 'is_approved', label: '状态', width: 90 },
  { key: 'actions', label: '操作', width: 190 }
]

/** 评论的复合主键：批量删除接口要求 slug + id */
function commentKey(c) {
  return `${c.slug}/${c.id}`
}

const commentFiltered = computed(() => {
  let list = comments.value
  if (onlyPending.value) list = list.filter(c => !c.is_approved)
  const k = commentKeyword.value.trim().toLowerCase()
  if (!k) return list
  return list.filter(
    c =>
      (c.author || '').toLowerCase().includes(k) ||
      (c.content || '').toLowerCase().includes(k) ||
      (c.slug || '').toLowerCase().includes(k)
  )
})

const commentPageRows = computed(() => {
  const start = (commentPage.value - 1) * pageSize
  return commentFiltered.value.slice(start, start + pageSize)
})

/**
 * 批量删除项目 / 工具 / 评论的公共流程。
 *
 * 输入 → 输出：选中的键数组 → 调用对应批量接口 → 汇总成功与失败条数
 * 关键机制：三个批量接口返回同一形状 { deleted, failed }，
 *           差异只在请求体（项目/工具是 slugs，评论是 items），用 bodyOf 适配
 * 不变量：失败项会逐条列出原因，不会静默吞掉
 * 边界：无选中直接返回；用户在 confirm 里取消则什么都不做
 *
 * 依据：
 *   经典问题：批量操作的部分失败（partial failure）
 *   本处变体：三张表共享同一套"尽量做、逐条汇报"的语义，只有请求体形状不同
 *   沿用结论：把汇报逻辑抽一份，避免三处重复且容易不一致
 *   偏离处：不做整体回滚，删文件不可逆（由部署备份兜底）
 */
async function runBatchDelete({ url, keys, bodyOf, label, reload }) {
  if (!keys.length) return
  if (!confirm(`把选中的 ${keys.length} ${label}移入垃圾箱？可以在「垃圾箱」里恢复。`)) return
  try {
    const res = await http.post(url, bodyOf(keys))
    const okCount = (res.deleted || []).length
    const failed = res.failed || []
    if (failed.length) {
      const detail = failed
        .map(f => `${f.slug ?? f.id ?? '?'}：${f.error}`)
        .join('\n')
      alert(`已删除 ${okCount} 项，失败 ${failed.length} 项：\n${detail}`)
    }
    await reload()
  } catch (e) {
    alert('批量删除失败：' + e.message)
  }
}

function batchDeleteToys(slugs) {
  return runBatchDelete({
    url: '/api/toys/batch-delete',
    keys: slugs,
    bodyOf: keys => ({ slugs: keys }),
    label: '个项目',
    reload: loadToys
  })
}

function batchDeleteTools(slugs) {
  return runBatchDelete({
    url: '/api/tools/batch-delete',
    keys: slugs,
    bodyOf: keys => ({ slugs: keys }),
    label: '个工具',
    reload: loadTools
  })
}

function batchDeleteComments(keys) {
  return runBatchDelete({
    url: '/api/comments/batch-delete',
    keys,
    bodyOf: ks => ({
      items: ks.map(k => {
        const [slug, ...rest] = String(k).split('/')
        return { slug, id: rest.join('/') }
      })
    }),
    label: '条评论',
    reload: loadComments
  })
}

const showArticleEditor = ref(false)
const showToyEditor = ref(false)
const showToolEditor = ref(false)

// 项目和工具已改为纯 Markdown 编辑（见下方的 toyText / toolText），
// 原先的编辑表单对象 emptyToy / emptyTool 与 joinList / splitList 已移除

const path = value => encodeURIComponent(value)

// ---------- 纯文本编辑器状态 ----------
//
// 设计：整个 .md 就是一个文本区，front matter 与 Markdown 都直接手写。
// 保存时把原文 POST 给后端，后端只做"能不能解析"的校验，不做结构体重排。

/** 编辑器里的完整文件内容 */
const rawText = ref('')
/** 打开时的原文，用于脏检查 */
const rawInitial = ref('')
/** 当前文件对应的 slug（改名后跟着变） */
const rawSlug = ref('')
/** 保存失败时的格式错误提示 */
const rawError = ref('')
/** 文本区 DOM */
const rawEditorRef = ref(null)
const toolEditorRef = ref(null)

// 项目 / 工具的纯 Markdown 编辑状态（与文章编辑器同一模式）
const toyText = ref('')
const toyInitial = ref('')
const toySlug = ref('')
const toyError = ref('')
const toyEditorRef = ref(null)

const toolText = ref('')
const toolInitial = ref('')
const toolSlug = ref('')
const toolError = ref('')

/** 项目编辑器是否有未保存改动 */
const toyDirty = computed(() => toyText.value !== toyInitial.value)
const toolDirty = computed(() => toolText.value !== toolInitial.value)

const rawLines = computed(() => (rawText.value ? rawText.value.split('\n').length : 0))

/** 当前是否有未保存的修改 */
const dirty = computed(() => showArticleEditor.value && rawText.value !== rawInitial.value)

/**
 * 全屏编辑器快捷键：Ctrl/Cmd+S 保存，Esc 返回。
 */
function onEditorKeydown(e) {
  if (!showArticleEditor.value) return
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault()
    if (!saving.value) saveRawArticle()
    return
  }
  if (e.key === 'Escape') {
    e.preventDefault()
    closeArticleEditor()
  }
}

onMounted(() => window.addEventListener('keydown', onEditorKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onEditorKeydown))

// ---------- 文章 ----------
async function loadArticles() {
  loading.value.articles = true
  try {
    const list = await http.get('/api/articles', { params: { limit: 200, fields: 'full' } })
    // 记录服务端当前别名：行内改名时要用它定位原文件，否则 PUT 会 404
    articles.value = (list || []).map(a => ({ ...a, originalSlug: a.slug }))
  } catch (e) {
    console.error('加载文章失败:', e)
  } finally {
    loading.value.articles = false
  }
}

/** 新建文章的初始模板：把常用字段摆好，省得每次手敲 */
function blankArticleTemplate() {
  const now = new Date().toISOString().slice(0, 19)
  return [
    '---',
    'title: ',
    'summary: ',
    'category: blog',
    'tags:',
    '- ',
    'cover_image: ',
    'is_published: false',
    `created_at: ${now}`,
    `updated_at: ${now}`,
    '---',
    '',
    ''
  ].join('\n')
}

/** 新建项目的初始模板（字段与 ToyRecord 一一对应） */
function blankToyTemplate() {
  const now = new Date().toISOString().slice(0, 19)
  return [
    '---',
    'name: ',
    'description: ',
    'image: ',
    'demo_url: ',
    'source_url: ',
    'tech_stack:',
    '- ',
    'is_published: false',
    `created_at: ${now}`,
    `updated_at: ${now}`,
    '---',
    '',
    '## 项目介绍',
    '',
    '这里写项目的详细说明，支持 Markdown 与代码块。',
    ''
  ].join('\n')
}

/** 新建工具的初始模板（字段与 ToolRecord 一一对应） */
function blankToolTemplate() {
  const now = new Date().toISOString().slice(0, 19)
  return [
    '---',
    'name: ',
    'description: ',
    'icon: 🔧',
    'category: utility',
    'url: ',
    'is_published: false',
    `created_at: ${now}`,
    `updated_at: ${now}`,
    '---',
    '',
    '## 工具说明',
    '',
    '介绍这个工具的用途、用法与注意事项。',
    ''
  ].join('\n')
}

/**
 * 从 Markdown 原文里取某个 front matter 字段的值。
 * 只用于新建时推导名称，不追求完整的 YAML 解析（真正校验在后端）。
 */
function frontMatterValue(raw, key) {
  const m = String(raw).match(new RegExp(`^\\s*${key}:\\s*(.+)$`, 'm'))
  if (!m) return ''
  return m[1].trim().replace(/^["']|["']$/g, '')
}

/**
 * 打开编辑器。
 *
 * 已有文章：拉 /raw 取原始全文（含 front matter），原样显示
 * 新建文章：填一份模板，slug 先由标题自动推导，保存时后端按 YAML 里的 title 生成
 */
async function openArticleEditor(item) {
  rawError.value = ''
  showArticleEditor.value = true

  if (!item) {
    rawSlug.value = ''
    rawText.value = blankArticleTemplate()
    rawInitial.value = rawText.value
    return
  }

  rawSlug.value = item.slug
  rawText.value = ''
  rawInitial.value = ''
  try {
    const data = await http.get(`/api/articles/${path(item.slug)}/raw`)
    rawText.value = data.raw || ''
    rawInitial.value = rawText.value
  } catch (e) {
    alert('读取原文失败：' + (e.message || '未知错误'))
    showArticleEditor.value = false
  }
}

/** 关闭编辑器：有未保存修改时先确认，避免误关丢失内容 */
function closeArticleEditor() {
  if (dirty.value && !confirm('有未保存的修改，确定离开吗？')) return
  showArticleEditor.value = false
  rawError.value = ''
}

/**
 * 在光标处插入 Markdown 片段。
 *
 * 输入 → 输出：片段类型 → 插入到选区位置并把光标移到待填处
 * 关键机制：直接操作 textarea 的 selectionStart/End，插入后派发 input 让 v-model 同步
 * 不变量：插入不会破坏已有内容，不会把选区外的文字弄丢
 * 边界：无焦点时追加到末尾
 */
function insertSnippet(kind) {
  // 交给 CodeMirror 编辑器处理：它在自己的文档模型里插入并维护光标
  rawEditorRef.value?.insertSnippet(kind)
}

/** 项目编辑器专用 */
function insertToySnippet(kind) {
  toyEditorRef.value?.insertSnippet(kind)
}

/** 工具编辑器专用（模板里直接绑定，语义更清楚） */
function insertToolSnippet(kind) {
  toolEditorRef.value?.insertSnippet(kind)
}

/**
 * 保存原文。
 *
 * 输入 → 输出：rawText → PUT /api/articles/{slug}/raw → 成功后刷新列表并关闭
 * 关键机制：后端先解析 YAML，解析失败返回 400 且**不写入**；
 *           前端把这条错误显示在状态条上，编辑器保持打开让你继续修
 * 不变量：保存失败时磁盘内容不变，本地文本也不丢
 * 边界：新建文章走同一接口（先 POST 创建再写原文），slug 由 YAML 里的 title 推导
 */
async function saveRawArticle() {
  rawError.value = ''
  saving.value = true
  try {
    if (rawSlug.value) {
      await http.put(`/api/articles/${path(rawSlug.value)}/raw`, { raw: rawText.value })
    } else {
      // 新建：先按 YAML 里的 title 建一条，再写入原文
      const titleMatch = rawText.value.match(/^\s*title:\s*(.+)$/m)
      const title = titleMatch ? titleMatch[1].trim().replace(/^["']|["']$/g, '') : ''
      if (!title) throw new Error('front matter 里必须有 title')
      const created = await http.post('/api/articles', {
        title,
        summary: '',
        category: 'blog',
        tags: [],
        cover_image: '',
        content: '',
        is_published: false
      })
      await http.put(`/api/articles/${path(created.slug)}/raw`, { raw: rawText.value })
    }
    rawInitial.value = rawText.value
    showArticleEditor.value = false
    await loadArticles()
  } catch (e) {
    rawError.value = e.message || '保存失败'
  } finally {
    saving.value = false
  }
}

// 粘贴/拖拽上传图片已由 MarkdownEditor 组件内部处理，
// 这里不再需要 onRawPaste / onRawDrop / uploadIntoRaw

async function deleteArticle(item) {
  if (!confirm(`把《${item.title}》移入垃圾箱？可以之后在「垃圾箱」里恢复。`)) return
  try {
    await http.delete(`/api/articles/${path(item.slug)}`)
    await loadArticles()
  } catch (e) {
    alert('操作失败：' + e.message)
  }
}

// ---------- 项目 ----------
async function loadToys() {
  loading.value.toys = true
  try {
    toys.value = await http.get('/api/toys', { params: { limit: 200 } })
  } catch (e) {
    console.error('加载项目失败:', e)
  } finally {
    loading.value.toys = false
  }
}

/**
 * 打开项目编辑器（纯 Markdown 模式，与文章一致）
 *
 * 已有项目：拉 /raw 取整篇 .md（front matter + 正文）原样显示
 * 新建项目：填一份字段模板，用户直接改 YAML
 */
async function openToyEditor(item) {
  toyError.value = ''
  toySlug.value = item?.slug || ''
  toyInitial.value = ''
  showToyEditor.value = true

  if (!item) {
    toyText.value = blankToyTemplate()
    toyInitial.value = toyText.value
    return
  }

  toyText.value = ''
  try {
    const data = await http.get(`/api/toys/${path(item.slug)}/raw`)
    toyText.value = data.raw || ''
    toyInitial.value = toyText.value
  } catch (e) {
    alert('读取原文失败：' + (e.message || '未知错误'))
    showToyEditor.value = false
  }
}

/** 保存项目原文 */
async function saveToy() {
  toyError.value = ''
  saving.value = true
  try {
    if (toySlug.value) {
      await http.put(`/api/toys/${path(toySlug.value)}/raw`, { raw: toyText.value })
    } else {
      // 新建：按 YAML 里的 name 创建记录，再写入原文
      const name = frontMatterValue(toyText.value, 'name')
      if (!name) throw new Error('front matter 里必须有 name')
      const created = await http.post('/api/toys', {
        name,
        description: frontMatterValue(toyText.value, 'description'),
        content: '',
        is_published: false
      })
      await http.put(`/api/toys/${path(created.slug)}/raw`, { raw: toyText.value })
    }
    toyInitial.value = toyText.value
    showToyEditor.value = false
    await loadToys()
  } catch (e) {
    toyError.value = e.message || '保存失败'
  } finally {
    saving.value = false
  }
}

/** 关闭项目编辑器（有改动时确认） */
function closeToyEditor() {
  if (toyText.value !== toyInitial.value && !confirm('有未保存的修改，确定离开吗？')) return
  showToyEditor.value = false
  toyError.value = ''
}

async function deleteToy(item) {
  if (!confirm(`把项目「${item.name}」移入垃圾箱？可以在「垃圾箱」里恢复。`)) return
  try {
    await http.delete(`/api/toys/${path(item.slug)}`)
    await loadToys()
  } catch (e) {
    alert('删除失败：' + e.message)
  }
}

// ---------- 工具 ----------
async function loadTools() {
  loading.value.tools = true
  try {
    tools.value = await http.get('/api/tools', { params: { limit: 200 } })
  } catch (e) {
    console.error('加载工具失败:', e)
  } finally {
    loading.value.tools = false
  }
}

/**
 * 打开工具编辑器（纯 Markdown 模式，与文章一致）
 */
async function openToolEditor(item) {
  toolError.value = ''
  toolSlug.value = item?.slug || ''
  toolInitial.value = ''
  showToolEditor.value = true

  if (!item) {
    toolText.value = blankToolTemplate()
    toolInitial.value = toolText.value
    return
  }

  toolText.value = ''
  try {
    const data = await http.get(`/api/tools/${path(item.slug)}/raw`)
    toolText.value = data.raw || ''
    toolInitial.value = toolText.value
  } catch (e) {
    alert('读取原文失败：' + (e.message || '未知错误'))
    showToolEditor.value = false
  }
}

/** 保存工具原文 */
async function saveTool() {
  toolError.value = ''
  saving.value = true
  try {
    if (toolSlug.value) {
      await http.put(`/api/tools/${path(toolSlug.value)}/raw`, { raw: toolText.value })
    } else {
      const name = frontMatterValue(toolText.value, 'name')
      if (!name) throw new Error('front matter 里必须有 name')
      const created = await http.post('/api/tools', {
        name,
        description: frontMatterValue(toolText.value, 'description'),
        content: '',
        is_published: false
      })
      await http.put(`/api/tools/${path(created.slug)}/raw`, { raw: toolText.value })
    }
    toolInitial.value = toolText.value
    showToolEditor.value = false
    await loadTools()
  } catch (e) {
    toolError.value = e.message || '保存失败'
  } finally {
    saving.value = false
  }
}

/** 关闭工具编辑器（有改动时确认） */
function closeToolEditor() {
  if (toolText.value !== toolInitial.value && !confirm('有未保存的修改，确定离开吗？')) return
  showToolEditor.value = false
  toolError.value = ''
}

async function deleteTool(item) {
  if (!confirm(`把工具「${item.name}」移入垃圾箱？可以在「垃圾箱」里恢复。`)) return
  try {
    await http.delete(`/api/tools/${path(item.slug)}`)
    await loadTools()
  } catch (e) {
    alert('删除失败：' + e.message)
  }
}

// ---------- 评论 ----------

async function loadComments() {
  loading.value.comments = true
  try {
    const data = await http.get('/api/comments')
    comments.value = data.items || []
  } catch (e) {
    console.error('加载评论失败:', e)
    comments.value = []
  } finally {
    loading.value.comments = false
  }
}

async function approveComment(item, approved) {
  try {
    await http.put(`/api/comments/${path(item.slug)}/${item.id}/approve`, { approved })
    await loadComments()
  } catch (e) {
    alert('操作失败：' + e.message)
  }
}

async function deleteComment(item) {
  if (!confirm(`确定删除 ${item.author} 在《${item.slug}》下的这条评论？`)) return
  try {
    await http.delete(`/api/comments/${path(item.slug)}/${item.id}`)
    await loadComments()
  } catch (e) {
    alert('删除失败：' + e.message)
  }
}

onMounted(() => {
  loadArticles()
  loadToys()
  loadTools()
  loadComments()
})</script>

<style scoped>
.tabs {
  display: flex;
  gap: 8px;
  margin-bottom: 24px;
  border-bottom: 1px solid var(--border);
}

.tab {
  padding: 10px 20px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  font-size: 0.95rem;
  font-family: inherit;
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
}

.tab:hover { color: var(--accent); }
.tab.active { color: var(--accent); border-bottom-color: var(--accent); }

.page-subtitle code {
  background: rgba(76, 201, 240, 0.1);
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 0.85em;
}

.toolbar {
  display: flex;
  gap: 10px;
  margin-bottom: 16px;
  align-items: center;
}

.data-table {
  width: 100%;
  border-collapse: collapse;
}

.data-table th, .data-table td {
  padding: 10px 12px;
  text-align: left;
  border-bottom: 1px solid var(--border);
}

.data-table th {
  color: var(--text-secondary);
  font-size: 0.85rem;
  font-weight: 600;
}

.data-table code {
  font-size: 0.8rem;
  color: var(--text-secondary);
}

.badge {
  background: rgba(76, 201, 240, 0.15);
  color: var(--accent);
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 0.8rem;
}

.status { font-size: 0.8rem; }
.status.published { color: var(--success); }
.status.draft { color: var(--warning); }

.actions { display: flex; gap: 6px; }
.btn-sm { padding: 4px 12px; font-size: 0.8rem; }

.filter-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.85rem;
  color: var(--text-secondary);
  cursor: pointer;
}

.comment-stat {
  margin-left: auto;
  font-size: 0.85rem;
  color: var(--text-secondary);
}

.comment-cell {
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---------- 表格里的单元格 ---------- */
.cell-title {
  display: block;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}

.loading-inline {
  font-size: 0.78rem;
  color: var(--text-muted);
  font-family: var(--font-mono);
}

.dt-danger-btn {
  padding: 0.2rem 0.7rem;
  background: rgba(233, 69, 96, 0.14);
  border: 1px solid rgba(233, 69, 96, 0.4);
  border-radius: 999px;
  color: #ff8fa3;
  font-size: 0.76rem;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.18s var(--ease);
}

.dt-danger-btn:hover {
  background: rgba(233, 69, 96, 0.26);
}

/* 工具名称前的 emoji 图标，与名称同一基线段落，不撑高行高 */
.cell-icon {
  margin-right: 0.4rem;
}

/* ---------- 文章表：行内编辑 ---------- */
/* 草稿标题降一点亮度，与"已上线"形成层次 */
.cell-title.is-draft {
  color: var(--text-secondary);
}

/* 草稿标签：明确告诉管理员"访客看不到这个" */
.draft-tag {
  display: inline-block;
  margin-left: 0.5rem;
  padding: 0.05rem 0.45rem;
  border-radius: 4px;
  font-size: 0.66rem;
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
  color: #fbbf24;
  background: rgba(251, 191, 36, 0.12);
  border: 1px solid rgba(251, 191, 36, 0.3);
  vertical-align: middle;
  white-space: nowrap;
}

.title-cell {
  display: flex;
  align-items: center;
  gap: 0.45rem;
}

/* 展开箭头：点开这一行才看到操作按钮 */
.chevron {
  display: inline-block;
  width: 0.8rem;
  flex-shrink: 0;
  color: var(--text-muted);
  font-size: 0.72rem;
  transition: transform 0.2s var(--ease);
}

.chevron.open {
  transform: rotate(90deg);
  color: var(--accent);
}

.inline-input,
.inline-select {
  width: 100%;
  padding: 0.24rem 0.45rem;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid transparent;
  border-radius: 6px;
  color: var(--text-primary);
  font-size: 0.82rem;
  font-family: inherit;
  transition: border-color 0.18s var(--ease), background 0.18s var(--ease);
}

.inline-input.mono {
  font-family: var(--font-mono);
  font-size: 0.78rem;
}

.inline-input:hover,
.inline-select:hover {
  border-color: var(--border);
}

.inline-input:focus,
.inline-select:focus {
  outline: none;
  border-color: var(--accent);
  background: rgba(76, 201, 240, 0.08);
}

/* 地址栏行内编辑：光标移上去才明显，避免整表都是输入框显得吵 */
.inline-input::placeholder {
  color: var(--text-muted);
}

/* 状态按钮：点一下即切换发布状态 */
.status-btn {
  padding: 0.18rem 0.6rem;
  border-radius: 999px;
  font-size: 0.74rem;
  font-family: inherit;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.18s var(--ease);
}

.status-btn.published {
  color: #6ee7a8;
  background: rgba(110, 231, 168, 0.1);
  border-color: rgba(110, 231, 168, 0.28);
}

.status-btn.draft {
  color: #fbbf24;
  background: rgba(251, 191, 36, 0.1);
  border-color: rgba(251, 191, 36, 0.28);
}

.status-btn:hover:not(:disabled) {
  filter: brightness(1.25);
}

.status-btn:disabled {
  opacity: 0.5;
  cursor: progress;
}

/* 展开后的详情块 */
.row-detail {
  margin-top: 0.55rem;
  padding: 0.6rem 0.75rem;
  border-left: 2px solid rgba(76, 201, 240, 0.35);
  background: rgba(76, 201, 240, 0.04);
  border-radius: 0 8px 8px 0;
}

.detail-line {
  display: flex;
  gap: 0.6rem;
  font-size: 0.79rem;
  color: var(--text-secondary);
  line-height: 1.7;
}

.detail-label {
  flex-shrink: 0;
  width: 2.6rem;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 0.72rem;
}

.detail-actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.55rem;
}

.btn-sm.btn-outline.danger {
  color: #ff8fa3;
  border-color: rgba(233, 69, 96, 0.35);
}

.btn-sm.btn-outline.danger:hover {
  background: rgba(233, 69, 96, 0.14);
}

/* ---------- 全屏文章编辑器（纯文本） ---------- */
/* 铺满视口，长文编辑时只有文本区在滚 */
.editor-page {
  position: fixed;
  inset: 0;
  z-index: 300;
  display: flex;
  flex-direction: column;
  background: #070a12;
}

.editor-bar {
  flex-shrink: 0;
  height: 52px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0 1.25rem;
  background: rgba(10, 14, 24, 0.96);
  border-bottom: 1px solid var(--border);
}

.editor-bar-left,
.editor-bar-right {
  display: flex;
  align-items: center;
  gap: 0.7rem;
}

.bar-back {
  background: none;
  border: none;
  padding: 0.3rem 0.5rem;
  color: var(--text-secondary);
  font-size: 0.85rem;
  font-family: inherit;
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.18s var(--ease);
}

.bar-back:hover {
  color: var(--accent);
  background: rgba(76, 201, 240, 0.08);
}

.bar-title {
  font-family: var(--font-serif);
  font-size: 0.98rem;
  font-weight: 600;
  color: #eef4ff;
}

.bar-dirty {
  font-size: 0.76rem;
  color: #fbbf24;
}

/* 单栏布局：文本区独占 */
.editor-body.single {
  flex: 1;
  display: flex;
  min-height: 0;
}

.editor-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  padding: 0;
}

/* 上方小工具条：文件名 + 常用片段按钮 */
.raw-toolbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.45rem 1.25rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  background: rgba(12, 16, 27, 0.5);
  font-size: 0.75rem;
  flex-wrap: wrap;
}

.raw-tab code {
  font-family: var(--font-mono);
  color: var(--accent-soft);
}

.raw-sep {
  color: var(--text-muted);
  opacity: 0.5;
}

.raw-btn {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 0.18rem 0.5rem;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 0.72rem;
  cursor: pointer;
  transition: all 0.16s var(--ease);
}

.raw-btn:hover {
  background: rgba(76, 201, 240, 0.12);
  border-color: rgba(76, 201, 240, 0.3);
  color: var(--accent);
}

.raw-tip {
  margin-left: auto;
  color: var(--text-muted);
  font-size: 0.72rem;
}

.content-wrap {
  flex: 1;
  min-height: 0;
  position: relative;
  display: flex;
}

/* 文本区：等宽字体、行号感的左内边距、内部滚动 */
.content-input.raw-input {
  flex: 1;
  width: 100%;
  padding: 1rem 1.25rem;
  background: transparent;
  border: none;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 0.86rem;
  line-height: 1.85;
  resize: none;
  tab-size: 2;
  white-space: pre;
  overflow: auto;
}

.content-input.raw-input:focus {
  outline: none;
}

.content-input.raw-input::placeholder {
  color: var(--text-muted);
  opacity: 0.7;
}

/* 底部状态条 */
/* ===== 元数据面板：编辑器下方的结构化字段 ===== */
.meta-panel {
  margin-top: 1rem;
  padding: 1.15rem 1.25rem;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: rgba(16, 22, 36, 0.55);
}

.meta-title {
  font-family: var(--font-sans);
  font-size: 0.82rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  color: var(--text-secondary);
  margin-bottom: 0.9rem;
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.meta-title::before {
  content: '';
  width: 2px;
  height: 0.9em;
  border-radius: 2px;
  background: var(--accent);
}

.meta-panel .form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.85rem;
}

.meta-panel .form-group {
  margin-bottom: 0.85rem;
}

.meta-panel .form-group:last-child {
  margin-bottom: 0;
}

.checkbox-group {
  display: flex;
  align-items: flex-end;
  padding-bottom: 0.35rem;
}

.checkbox-label {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.85rem;
  color: var(--text-secondary);
  cursor: pointer;
}

@media (max-width: 720px) {
  .meta-panel .form-row {
    grid-template-columns: 1fr;
  }
}
.editor-status {
  flex-shrink: 0;
  padding: 0.45rem 1.25rem;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  font-family: var(--font-mono);
  font-size: 0.72rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.status-sep {
  opacity: 0.5;
}

/* 保存失败的格式错误：红色显眼，留在状态条上让你继续改 */
.status-error {
  margin-left: auto;
  color: #ff8fa3;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 60%;
}

@media (max-width: 720px) {
  .raw-tip {
    display: none;
  }

  .content-input.raw-input {
    font-size: 0.8rem;
    padding: 0.75rem;
  }
}


/* ---------- 通用弹窗（项目 / 工具仍用弹窗） ---------- */
.modal-overlay {
  position: fixed;
  top: 0; left: 0; right: 0; bottom: 0;
  /* 遮罩加深 + 模糊：后面表格里的文字不再透上来干扰阅读 */
  background: rgba(2, 4, 9, 0.88);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  z-index: 1000;
  padding: 40px 20px;
  overflow-y: auto;
}

.modal {
  /* 不透明底色：即使遮罩被浏览器降级，弹窗本身也不会透出后面的内容 */
  background: #0e1320;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 14px;
  padding: 30px;
  width: 100%;
  max-width: 700px;
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.65);
}

.modal h2 { margin-bottom: 20px; }

.form-group { margin-bottom: 14px; }
.form-group label { display: block; margin-bottom: 4px; font-size: 0.9rem; color: var(--text-secondary); }
.form-row { display: flex; gap: 14px; }
.form-row .form-group { flex: 1; }

.form-input, .form-textarea {
  width: 100%;
  padding: 10px 12px;
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-primary);
  font-size: 0.9rem;
  font-family: inherit;
}

.form-textarea { resize: vertical; }
.form-input:focus, .form-textarea:focus { outline: none; border-color: var(--accent); }

.form-actions { display: flex; gap: 10px; margin-top: 20px; }

/* ---------- 粘贴上传 ---------- */
/* 图片类输入框（封面/图片 URL）的粘贴提示 */
.form-input.pasteable {
  border-style: dashed;
}

.form-input.pasteable:focus {
  border-style: solid;
}

.paste-hint {
  font-size: 0.72rem;
  color: var(--text-muted);
  margin-top: 0.3rem;
}

.paste-hint.busy {
  color: var(--accent);
}

/* 编辑器上盖一层"拖拽或粘贴"提示 */
.paste-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  background: rgba(6, 10, 20, 0.72);
  border: 1px dashed rgba(76, 201, 240, 0.5);
  color: var(--accent-soft);
  font-size: 0.85rem;
  pointer-events: none;
  z-index: 3;
}

.field-relative {
  position: relative;
}

@media (max-width: 768px) {
  .data-table { display: block; overflow-x: auto; }
  .form-row { flex-direction: column; gap: 0; }
}
</style>
