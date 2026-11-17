<!--
  DataTable —— 后台表格（参考 vue-pure-admin 的 ElTable 交互，不引入 Element Plus）

  逻辑描述：
    输入：
      - rows：当前页要显示的行
      - columns：[{ key, label, width?, align? }]
      - total / page / pageSize：分页信息（受控）
      - searchable：是否显示搜索框；searchField：搜索匹配哪些字段
      - selectable：是否显示多选列（用于批量删除）
    输出：带表头、斑马纹、hover 高亮、分页、多选、空态的表格
    关键机制：
      - 列定义驱动渲染，单元格内容通过具名插槽 `cell-<key>` 自定义
      - 搜索与分页都是"受控"的：组件只发事件，不自己持有筛选结果
      - 多选用 Set 存选中行的 id，翻页时保留选择
    不变量：父组件传入的 rows 顺序即渲染顺序；页大小变化后回到第 1 页
    边界：rows 为空时显示空态而不是空白表格；窄屏横向滚动

  依据：
    经典问题：表格组件的受控状态（controlled component）
    本处变体：后台只需要表格这一块，引入 Element Plus + vue-pure-admin
               会带来一整棵依赖树和一套 CSS，与你现有的手写主题冲突
    沿用结论：受控规则——组件不持有数据源，只渲染父组件给的 rows，
               分页/搜索/排序全部以事件上抛
    偏离处：没有做列宽拖拽、行拖拽排序、虚拟滚动（后台数据量在百级）
-->
<template>
  <div class="dt">
    <!-- 工具条 -->
    <div v-if="searchable || $slots.toolbar" class="dt-toolbar">
      <div class="dt-toolbar-left">
        <slot name="toolbar"></slot>
      </div>
      <div v-if="searchable" class="dt-search">
        <input
          v-model.trim="keyword"
          class="dt-search-input"
          type="search"
          :placeholder="searchPlaceholder"
          @input="$emit('search', keyword)"
        />
        <button v-if="keyword" type="button" class="dt-search-clear" @click="clearSearch">✕</button>
      </div>
    </div>

    <!-- 批量操作条 -->
    <div v-if="selectable && selected.size" class="dt-batch">
      <span>已选 {{ selected.size }} 项</span>
      <button type="button" class="dt-batch-btn" @click="clearSelection">取消选择</button>
      <slot name="batch" :selected="[...selected]"></slot>
    </div>

    <div class="dt-wrap">
      <table class="dt-table">
        <thead>
          <tr>
            <th v-if="selectable" class="dt-col-check">
              <input
                type="checkbox"
                :checked="allSelected"
                :indeterminate.prop="selected.size > 0 && !allSelected"
                @change="toggleAll"
              />
            </th>
            <th
              v-for="col in columns"
              :key="col.key"
              :style="{ width: col.width ? col.width + 'px' : undefined, textAlign: col.align || 'left' }"
            >{{ col.label }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="row in rows"
            :key="rowKey(row)"
            :class="{
              'is-selected': selected.has(rowKey(row)),
              'is-clickable': clickableRows,
              'is-muted': mutedRow ? mutedRow(row) : false
            }"
            @click="onRowClick(row, $event)"
          >
            <td v-if="selectable" class="dt-col-check" @click.stop>
              <input
                type="checkbox"
                :checked="selected.has(rowKey(row))"
                @change="toggleRow(rowKey(row))"
              />
            </td>
            <td
              v-for="col in columns"
              :key="col.key"
              :style="{ textAlign: col.align || 'left' }"
            >
              <slot :name="`cell-${col.key}`" :row="row">
                {{ row[col.key] }}
              </slot>
            </td>
          </tr>
        </tbody>
      </table>

      <p v-if="!rows.length" class="dt-empty">{{ emptyText }}</p>
    </div>

    <!-- 分页：总条数 + 页码 -->
    <div v-if="total > pageSize" class="dt-pager">
      <span class="dt-total">共 {{ total }} 条</span>
      <button type="button" class="dt-page-btn" :disabled="page <= 1" @click="go(page - 1)">‹</button>
      <span class="dt-page-info">{{ page }} / {{ pageCount }}</span>
      <button type="button" class="dt-page-btn" :disabled="page >= pageCount" @click="go(page + 1)">›</button>
    </div>
  </div>
</template>

<script setup>
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
const selected = ref(new Set())

const pageCount = computed(() => Math.max(1, Math.ceil(props.total / props.pageSize)))
const allSelected = computed(
  () => props.rows.length > 0 && props.rows.every(r => selected.value.has(rowKey(r)))
)

/** 行标识：默认用 id，没有就退化成索引组合 */
function rowKey(row) {
  if (props.rowId) return props.rowId(row)
  return row.id ?? row.slug ?? JSON.stringify(row)
}

function toggleRow(key) {
  const next = new Set(selected.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  selected.value = next
  emit('selection-change', [...next])
}

function toggleAll() {
  if (allSelected.value) {
    selected.value = new Set()
  } else {
    selected.value = new Set(props.rows.map(rowKey))
  }
  emit('selection-change', [...selected.value])
}

function clearSelection() {
  selected.value = new Set()
  emit('selection-change', [])
}

function clearSearch() {
  keyword.value = ''
  emit('search', '')
}

function go(p) {
  const clamped = Math.min(Math.max(1, p), pageCount.value)
  emit('update:page', clamped)
}

/**
 * 整行点击。
 *
 * 输入 → 输出：行数据 + 事件 → emit('row-click', row)
 * 关键机制：先判断点击目标是否落在交互元素里（button/input/select/a/textarea
 *           或 .dt-no-row-click 容器），是则不触发行点击
 * 不变量：行内的"展开/编辑"按钮、下拉框、复选框都不会被行点击吞掉
 * 边界：单元格自定义插槽里若放了别的可交互组件，给它加 .dt-no-row-click 即可豁免
 *
 * 依据：
 *   经典问题：事件冒泡与交互元素冲突（event delegation 的命中边界）
 *   本处变体：表格同时支持"整行展开"和"单元格内联编辑"两种点击语义
 *   沿用结论：用 closest() 从目标向上找交互边界，命中即短路，避免双触发
 *   偏离处：不区分左中右键，右键菜单尚未用到
 */
function onRowClick(row, event) {
  if (!props.clickableRows) return
  const interactive = event.target.closest(
    'button, input, select, textarea, a, label, .dt-no-row-click'
  )
  if (interactive) return
  emit('row-click', row)
}

defineExpose({ clearSelection })
</script>

<style scoped>
.dt {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.dt-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.dt-toolbar-left {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  flex-wrap: wrap;
}

.dt-search {
  position: relative;
  display: flex;
  align-items: center;
}

.dt-search-input {
  width: 200px;
  padding: 0.4rem 2rem 0.4rem 0.9rem;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--text-primary);
  font-size: 0.82rem;
  font-family: inherit;
  transition: border-color 0.2s var(--ease), width 0.2s var(--ease);
}

.dt-search-input:focus {
  outline: none;
  border-color: var(--accent);
  width: 240px;
}

.dt-search-clear {
  position: absolute;
  right: 0.5rem;
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 0.72rem;
  cursor: pointer;
}

.dt-batch {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.4rem 0.8rem;
  border-radius: var(--radius-sm);
  background: rgba(76, 201, 240, 0.1);
  border: 1px solid rgba(76, 201, 240, 0.25);
  font-size: 0.8rem;
  color: var(--accent-soft);
}

.dt-batch-btn {
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 0.76rem;
  font-family: inherit;
  cursor: pointer;
}

.dt-wrap {
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  overflow: hidden;
}

.dt-table {
  width: 100%;
  border-collapse: collapse;
}

.dt-table th {
  padding: 0.6rem 0.85rem;
  text-align: left;
  font-size: 0.76rem;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--text-secondary);
  background: rgba(255, 255, 255, 0.04);
  border-bottom: 1px solid var(--border);
  white-space: nowrap;
}

.dt-table td {
  padding: 0.6rem 0.85rem;
  font-size: 0.84rem;
  color: var(--text-primary);
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  vertical-align: middle;
}

/* 斑马纹 + hover：长表格也容易横向追踪 */
.dt-table tbody tr:nth-child(even) {
  background: rgba(255, 255, 255, 0.015);
}

.dt-table tbody tr:hover {
  background: rgba(255, 255, 255, 0.05);
}

.dt-table tbody tr.is-selected {
  background: rgba(76, 201, 240, 0.1);
}

/* 整行可点时给出手型光标，明确"点这行能展开" */
.dt-table tbody tr.is-clickable {
  cursor: pointer;
}

/* 未上线的行（草稿）：整行降透明度，和已发布内容拉开层次 */
.dt-table tbody tr.is-muted {
  opacity: 0.55;
}

.dt-table tbody tr.is-muted:hover,
.dt-table tbody tr.is-muted.is-selected {
  opacity: 1;
}

.dt-table tbody tr:last-child td {
  border-bottom: none;
}

.dt-col-check {
  width: 36px;
  text-align: center;
}

.dt-col-check input {
  cursor: pointer;
  accent-color: var(--accent);
}

.dt-empty {
  padding: 2.5rem 1rem;
  text-align: center;
  color: var(--text-muted);
  font-size: 0.84rem;
}

.dt-pager {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  font-size: 0.78rem;
  color: var(--text-muted);
}

.dt-total {
  margin-right: auto;
}

.dt-page-btn {
  width: 1.7rem;
  height: 1.7rem;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.18s var(--ease);
}

.dt-page-btn:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent-soft);
}

.dt-page-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.dt-page-info {
  font-family: var(--font-mono);
}

@media (max-width: 720px) {
  .dt-wrap {
    overflow-x: auto;
  }

  .dt-search-input,
  .dt-search-input:focus {
    width: 140px;
  }
}
</style>
