/**
 * 后台表格的 schema 配置
 *
 * 逻辑描述：
 *   输入 → 输出：资源定义（表名 + 字段元信息 + 接口路径）→ 表格与表单的渲染依据
 *   关键机制：
 *     1. 每张表在一个对象里声明：列、可搜索字段、可编辑字段、API 路径；
 *     2. 表格列、搜索逻辑、行内编辑表单全部从这份配置派生；
 *     3. 新增一张表只需加一个对象，不再复制粘贴四遍「列定义 + 过滤 + 分页」。
 *   不变量：
 *     - 列的 key 必须与后端返回的 JSON 字段同名；
 *     - 可编辑字段的 type 决定表单控件（text / switch / tags / select）。
 *   边界：
 *     - 只覆盖"列表 + 行内编辑"；复杂表单（如文章正文编辑器）仍是独立页面；
 *     - 不处理后端字段重命名——改了配置就要同步改后端。
 *
 * 依据：
 *   经典问题：配置驱动 UI（schema-driven / metadata-driven UI）
 *   来源：
 *     1. cinvue —— 元数据驱动的 CRUD 页面
 *        https://github.com/amirmf404/cinvue
 *        原文："ships metadata-driven CRUD pages (table, form, view),
 *               dynamic inputs, and supporting composables/stores so apps can
 *               render admin-style UIs from model definitions"
 *     2. Me-Admin —— 从实体定义生成前端页面
 *        https://github.com/meadmin-cn/meadmin
 *        原文："Define a Sequelize entity, run `meadmin crud <entity> --menu`
 *               and a complete CRUD module... is generated as plain readable source code"
 *   原型结论：
 *     1. 后台 CRUD 页面的结构高度同质，用配置描述比手写重复模板更易维护；
 *     2. 两个项目都验证了"模型 → 表格/表单"这条路径可行。
 *   本处变体：
 *     - 不引入这两个框架（一个是 Vue+PrimeVue，一个是 Node 全栈，
 *       都与本项目 Element Plus + Rust 的栈冲突）；
 *     - 只借用其**思路**：把每张表的字段元信息抽成配置。
 *   沿用结论：用配置对象驱动表格列与行内编辑表单；
 *             Admin.vue 里四份重复的列定义合并为一份配置。
 *   偏离处：
 *     - 不做代码生成（Me-Admin 那样生成源文件）——
 *       本项目只有 4 张表，运行时读配置比生成代码简单，且配置改了立刻生效；
 *     - 不做自动的表单校验规则生成，校验仍在后端。
 */

/**
 * 字段类型 → 表格渲染与表单控件
 *
 *   text     纯文本（表格直接显示，表单用 el-input）
 *   badge    标签样式的短文本（表格用 el-tag，表单用 el-input）
 *   date     ISO 时间字符串（表格格式化显示，表单只读）
 *   bool     布尔（表格用 el-tag 显示状态，表单用 el-switch）
 *   tags     字符串数组（表格用多个 el-tag，表单用逗号分隔输入）
 *   number   数字
 *   actions  操作按钮列（仅表格，不参与表单）
 */
export const FIELD_TYPES = ['text', 'badge', 'date', 'bool', 'tags', 'number', 'actions']

/**
 * 文章表
 */
export const articlesSchema = {
  key: 'articles',
  label: '文章',
  /** 后端接口前缀 */
  api: '/api/articles',
  /** 行标识字段（用于多选与批量删除） */
  rowKey: 'slug',
  /** 主键字段名（新建/更新时用） */
  primaryKey: 'slug',
  /** 搜索时匹配的字段 */
  searchFields: ['title', 'slug'],
  columns: [
    { key: 'title', label: '标题', type: 'text', editable: true },
    { key: 'slug', label: '别名', type: 'text', width: 170, editable: true, placeholder: '英文与短横线' },
    { key: 'category', label: '分类', type: 'badge', width: 120, editable: true, placeholder: '如 blog' },
    { key: 'is_published', label: '状态', type: 'bool', width: 100, editable: true },
    { key: 'created_at', label: '日期', type: 'date', width: 110, editable: false }
  ],
  /** 批量删除接口（POST，body 形如 { slugs: [...] }） */
  batchDeletePath: '/api/articles/batch-delete',
  batchDeleteBodyKey: 'slugs'
}

/**
 * 项目表
 */
export const toysSchema = {
  key: 'toys',
  label: '项目',
  api: '/api/toys',
  rowKey: 'slug',
  primaryKey: 'slug',
  searchFields: ['name', 'slug'],
  columns: [
    { key: 'name', label: '名称', type: 'text', editable: true },
    { key: 'slug', label: '别名', type: 'text', width: 150, editable: true },
    { key: 'tech_stack', label: '技术栈', type: 'tags', editable: true, placeholder: '逗号分隔' },
    { key: 'is_published', label: '状态', type: 'bool', width: 90, editable: true },
    { key: 'actions', label: '操作', type: 'actions', width: 130 }
  ],
  batchDeletePath: '/api/toys/batch-delete',
  batchDeleteBodyKey: 'slugs'
}

/**
 * 工具表
 */
export const toolsSchema = {
  key: 'tools',
  label: '工具',
  api: '/api/tools',
  rowKey: 'slug',
  primaryKey: 'slug',
  searchFields: ['name', 'slug'],
  columns: [
    { key: 'name', label: '名称', type: 'text', editable: true },
    { key: 'slug', label: '别名', type: 'text', width: 150, editable: true },
    { key: 'category', label: '分类', type: 'badge', width: 110, editable: true },
    { key: 'is_published', label: '状态', type: 'bool', width: 90, editable: true },
    { key: 'actions', label: '操作', type: 'actions', width: 130 }
  ],
  batchDeletePath: '/api/tools/batch-delete',
  batchDeleteBodyKey: 'slugs'
}

/**
 * 评论表
 *
 * 评论的特殊性：
 *   - 跨文章（每行带 slug 表示属于哪篇）
 *   - 审核是个动作，不是字段直接改
 */
export const commentsSchema = {
  key: 'comments',
  label: '评论',
  api: '/api/comments',
  rowKey: 'id',
  primaryKey: 'id',
  /** 评论按 (slug, id) 定位，批量删除特殊处理 */
  compositeKey: ['slug', 'id'],
  searchFields: ['author', 'content'],
  columns: [
    { key: 'slug', label: '文章', type: 'badge', width: 150, editable: false },
    { key: 'author', label: '作者', type: 'text', width: 120, editable: false },
    { key: 'content', label: '内容', type: 'text', editable: false },
    { key: 'is_approved', label: '审核', type: 'bool', width: 90, editable: false },
    { key: 'created_at', label: '日期', type: 'date', width: 110, editable: false }
  ],
  /** 审核接口：PUT /api/comments/{slug}/{id}/approve */
  approvePath: (row) => `/api/comments/${encodeURIComponent(row.slug)}/${encodeURIComponent(row.id)}/approve`,
  /** 删除接口：DELETE /api/comments/{slug}/{id} */
  deletePath: (row) => `/api/comments/${encodeURIComponent(row.slug)}/${encodeURIComponent(row.id)}`
}

/**
 * 全部 schema，按后台 Tab 顺序排列
 */
export const ADMIN_SCHEMAS = {
  articles: articlesSchema,
  toys: toysSchema,
  tools: toolsSchema,
  comments: commentsSchema
}

/**
 * 从 schema 生成「可编辑字段」列表
 *
 * 用于行内编辑表单：只渲染 editable=true 且非 actions 的列。
 */
export function editableFields(schema) {
  return schema.columns.filter((c) => c.editable && c.type !== 'actions')
}

/**
 * 从 schema 生成表格列（剔除 actions，它由插槽渲染）
 */
export function tableColumns(schema) {
  return schema.columns.filter((c) => c.type !== 'actions')
}

/**
 * 按 schema 的 searchFields 过滤行
 *
 * 关键机制：忽略大小写，任一字段命中即保留。
 * 与原来的手写过滤逻辑等价，只是字段来源改由配置决定。
 */
export function filterRows(schema, rows, keyword) {
  const k = (keyword || '').trim().toLowerCase()
  if (!k) return rows

  return rows.filter((row) =>
    schema.searchFields.some((field) => {
      const v = row[field]
      return v != null && String(v).toLowerCase().includes(k)
    })
  )
}
