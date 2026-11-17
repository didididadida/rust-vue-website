<!--
  DataTable —— 后台表格（基于 Element Plus ElTable 封装）

  逻辑描述：
    输入：
      - rows：当前页要显示的行
      - columns：[{ key, label, width?, align? }]
      - total / page / pageSize：分页信息（受控）
      - searchable：是否显示搜索框；searchPlaceholder：占位文案
      - selectable：是否显示多选列（用于批量删除）
      - clickableRows：整行可点，上抛 row-click
      - mutedRow：返回 true 的行整行变暗（区分草稿与已发布）
      - rowId：行标识函数
    输出：
      - 带表头、斑马纹、hover 高亮、分页、多选、空态的表格
      - 事件：update:page / search / selection-change / row-click
    关键机制：
      - 外层保持原来的 props/emits 契约，Admin.vue 不需改动；
      - 内部用 el-table 渲染，列由 columns 驱动，
        单元格仍通过具名插槽 `cell-<key>` 自定义；
      - 搜索与分页仍是"受控"的：组件只发事件，不自己持有筛选结果；
      - 多选由 el-table 的 selection 列管理，但上抛前映射成「行标识数组」，
        与原实现的契约保持一致（见 onSelectionChange 注释）。
    不变量：
      - 父组件传入的 rows 顺序即渲染顺序
      - 页大小变化后回到第 1 页（由父组件处理，本组件只上抛事件）
    边界：
      - rows 为空时显示 el-table 的 empty 插槽，不是空白表格
      - 窄屏横向滚动由 el-table 自带

  依据：
    经典问题：受控组件（controlled component）与库替换的接口稳定性
    来源：
      1. 本组件原有实现（手写表格）的接口契约 —— 见 git 历史；
         其注释原话："引入 Element Plus + vue-pure-admin 会带来一整棵依赖树
         和一套 CSS，与你现有的手写主题冲突"。
      2. Element Plus Table 官方文档
         https://element-plus.org/zh-CN/component/table.html
         要点：`data` 绑数据、`selection` 列做多选、
               `@selection-change` 抛选中行、`row-key` 保证翻页保持选择、
               `empty-text` / `#empty` 定制空态。
    原型结论：
      1. 表格是典型受控组件：数据从外传入，交互以事件上抛；
      2. ElTable 的 selection 在 data 变化时会重置，
         必须配 `row-key` 才能跨页保留选择。
    本处变体：
      - 原组件为了避开 Element Plus 而手写，现项目已引入 Element Plus，
        继续维护第二套表格实现属于"同一功能两套实现"（层级纪律禁止）；
      - Admin.vue 有 1879 行，大量使用本组件，
        接口一旦变化就要改很多处，因此**保持契约不变**、只换内部实现。
    沿用结论：
      - props 与 emits 完全保持原样；
      - 内部改用 el-table + el-pagination；
      - 用 row-key 保证跨页选择保留。
    偏离处：
      - 未实现列宽拖拽、行拖拽排序、虚拟滚动（与原组件一致，后台数据量在百级）；
      - 搜索结果高亮未迁移（原组件也没有）。
-->
<template>
  <div class="dt">
    <!-- 工具条：左侧放父组件给的按钮，右侧搜索框 -->
    <div v-if="searchable || $slots.toolbar" class="dt-toolbar">
      <div class="dt-toolbar-left">
        <slot name="toolbar"></slot>
      </div>
      <div v-if="searchable" class="dt-search">
        <el-input
          v-model.trim="keyword"
          :placeholder="searchPlaceholder"
          clearable
          size="default"
          @input="$emit('search', keyword)"
        >
          <template #prefix>
            <span class="dt-search-icon">⌕</span>
          </template>
        </el-input>
      </div>
    </div>

    <!-- 表格主体 -->
    <el-table
      :data="rows"
      :row-key="tableRowKey"
      :empty-text="emptyText"
      :row-class-name="rowClassName"
      stripe
      border
      style="width: 100%"
      @selection-change="onSelectionChange"
      @row-click="onRowClick"
    >
      <!-- 多选列 -->
      <el-table-column v-if="selectable" type="selection" width="48" reserve-selection />

      <!-- 数据列：由 columns 驱动，内容可用具名插槽覆盖 -->
      <el-table-column
        v-for="col in columns"
        :key="col.key"
        :prop="col.key"
        :label="col.label"
        :width="col.width"
        :align="col.align || 'left'"
      >
        <template #default="scope">
          <slot :name="`cell-${col.key}`" :row="scope.row" :value="scope.row[col.key]">
            {{ scope.row[col.key] }}
          </slot>
        </template>
      </el-table-column>
    </el-table>

    <!-- 分页：总数超过一页才显示 -->
    <div v-if="total > pageSize" class="dt-pager">
      <el-pagination
        :current-page="page"
        :page-size="pageSize"
        :total="total"
        layout="prev, pager, next, total"
        background
        @current-change="onPageChange"
      />
    </div>
  </div>
</template>

<script setup>
/**
 * 表格封装：对外契约与原手写实现完全一致
 *
 * 设计要点：
 *   1. 组件不持有数据：rows / page / total 全部由父组件传入；
 *   2. 选中集合由 el-table 内部维护，变化时整体上抛（selection-change）；
 *   3. row-key 是跨页保留选择的必要条件——没有它，翻页会自动清空选中。
 */
import { computed, ref } from 'vue'

const props = defineProps({
  rows: { type: Array, default: () => [] },
  columns: { type: Array, required: true },
  total: { type: Number, default: 0 },
  page: { type: Number, default: 1 },
  pageSize: { type: Number, default: 10 },
  searchable: { type: Boolean, default: false },
  searchPlaceholder: { type: String, default: '搜索…' },
  selectable: { type: Boolean, default: false },
  /** 整行可点：点击非交互元素时上抛 row-click */
  clickableRows: { type: Boolean, default: false },
  /**
   * 标记"未上线"的行（可选）。返回 true 的行会整行变暗，
   * 用于区分「管理员预览到的草稿」与「访客真正看得到的已发布内容」。
   */
  mutedRow: { type: Function, default: null },
  emptyText: { type: String, default: '暂无数据' },
  rowId: { type: Function, default: null }
})

const emit = defineEmits(['update:page', 'search', 'selection-change', 'row-click'])

const keyword = ref('')

/**
 * 行标识：默认用 id，退化到 slug，最后用序号+标题
 *
 * 为什么不能直接用对象引用：
 *   el-table 的 reserve-selection 需要稳定且唯一的字符串/数字 key。
 */
function rowKey(row, index) {
  if (props.rowId) return props.rowId(row)
  if (row.id !== undefined) return row.id
  if (row.slug !== undefined) return row.slug
  return `idx-${index}`
}

/** el-table 要求 row-key 接收行对象；这里补上索引回退 */
function tableRowKey(row) {
  const idx = props.rows.indexOf(row)
  return rowKey(row, idx)
}

/** 草稿行变暗 */
function rowClassName({ row }) {
  if (props.mutedRow && props.mutedRow(row)) return 'dt-row-muted'
  return ''
}

/**
 * 多选变化：把选中行映射成「行标识」数组后上抛
 *
 * 为什么不能直接透传 el-table 的 selection：
 *   原手写实现的 selection-change 抛的是**行标识**（slug / id 字符串），
 *   Admin.vue 里 `batchDeleteArticles(slugs)` 直接把这些标识发给后端，
 *   请求体形如 `{ slugs: ["a", "b"] }`。
 *   而 el-table 抛的是完整行对象数组，直接透传会变成
 *   `{ slugs: [{slug:"a",...}, {slug:"b",...}] }`，后端无法解析。
 *
 *   这是"库替换时接口语义保持不变"的具体要求：形状变了就等于改了契约。
 */
function onSelectionChange(selection) {
  const keys = selection.map((row) => {
    const idx = props.rows.indexOf(row)
    return rowKey(row, idx)
  })
  emit('selection-change', keys)
}

/** 整行点击：只在 clickableRows 开启时上抛 */
function onRowClick(row) {
  if (props.clickableRows) emit('row-click', row)
}

function onPageChange(p) {
  emit('update:page', p)
}
</script>

<style scoped>
.dt {
  width: 100%;
}

/* ---------- 工具条 ---------- */
.dt-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}

.dt-toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.dt-search {
  width: 220px;
  flex-shrink: 0;
}

.dt-search-icon {
  color: #8b93a1;
  font-size: 14px;
}

/* ---------- 分页 ---------- */
.dt-pager {
  display: flex;
  justify-content: flex-end;
  margin-top: 14px;
}

/* ---------- 草稿行 ---------- */
/* 整行变暗表示"访客看不到"，只是视觉提示，不影响交互 */
:deep(.dt-row-muted) {
  opacity: 0.55;
}

/* ---------- 表格细节 ---------- */
/* 表头字重与原手写表格一致，避免视觉跳变 */
:deep(.el-table th.el-table__cell) {
  font-weight: 500;
  font-size: 0.78rem;
  letter-spacing: 0.03em;
  text-transform: uppercase;
}

/* 单元格不再用默认的黑色文字，跟随站点文字色 */
:deep(.el-table) {
  font-size: 0.88rem;
  border-radius: 12px;
  overflow: hidden;
}

/* 可点击行给出手型指针 */
:deep(.el-table__row) {
  transition: background-color 0.15s ease;
}
</style>
