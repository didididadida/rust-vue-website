<!--
  AvatarCropper —— 极简头像裁剪（拖动选位置，滚轮缩放）

  【怎么用】
    1. 上传一张图
    2. 在圆框里拖动，把想露出来的部分挪到中间
    3. 滚轮或捏合缩放
    4. 保存

  【为什么只存三个数】
    不裁剪文件、不生成新图。只记「放大到几倍、往哪偏」，
    渲染时用 CSS transform 复现同一视角。
    原图永远只有一份，改裁剪不用重新上传。

  依据：
    经典问题：非破坏性编辑（non-destructive editing）
    本处变体：没有服务端图像处理能力，也不该为头像存多份副本
    沿用结论：参数化视图，原图 + (zoom, offsetX, offsetY) 即可完整复现
    偏离处：偏移用百分比而非像素，保证 96px 和 180px 下裁剪位置一致
-->
<template>
  <div class="cropper">
    <div
      class="stage"
      :class="{ dragging }"
      @pointerdown="onDown"
      @pointermove="onMove"
      @pointerup="onUp"
      @pointercancel="onUp"
      @wheel.prevent="onWheel"
    >
      <img
        v-if="src"
        :src="src"
        class="photo"
        :style="photoStyle"
        draggable="false"
        @load="onLoad"
      />
      <div v-else class="hint-empty">请先选择一张图片</div>
      <div class="shade"></div>
    </div>

    <div class="bar">
      <input
        :value="zoom"
        type="range"
        min="1"
        max="4"
        step="0.02"
        class="zoom"
        @input="$emit('update:zoom', Number($event.target.value))"
      />
      <button type="button" class="reset" @click="reset">复位</button>
    </div>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'

const props = defineProps({
  src: { type: String, default: '' },
  zoom: { type: Number, default: 1 },
  offsetX: { type: Number, default: 0 },
  offsetY: { type: Number, default: 0 }
})

const emit = defineEmits(['update:zoom', 'update:offsetX', 'update:offsetY'])

const stage = ref(null)
const dragging = ref(false)
const size = ref({ w: 1, h: 1 })

let start = null

/**
 * 图片的样式：cover 铺满方框（短边贴边），再叠用户缩放与偏移。
 * 用 cover 而不是 contain，是因为头像框是圆的，正方形外接框铺满才没有黑边。
 */
const photoStyle = computed(() => {
  const z = Number(props.zoom) || 1
  return {
    transform: `translate(${props.offsetX}%, ${props.offsetY}%) scale(${z})`
  }
})

function onLoad(e) {
  size.value = { w: e.target.naturalWidth || 1, h: e.target.naturalHeight || 1 }
}

function onDown(e) {
  if (!props.src) return
  dragging.value = true
  const rect = stage.value.getBoundingClientRect()
  start = { x: e.clientX, y: e.clientY, ox: props.offsetX, oy: props.offsetY, rect }
  stage.value.setPointerCapture?.(e.pointerId)
}

function onMove(e) {
  if (!dragging.value || !start) return
  const dx = ((e.clientX - start.x) / start.rect.width) * 100
  const dy = ((e.clientY - start.y) / start.rect.height) * 100
  emit('update:offsetX', clamp(start.ox + dx))
  emit('update:offsetY', clamp(start.oy + dy))
}

function onUp(e) {
  dragging.value = false
  start = null
  try {
    stage.value?.releasePointerCapture?.(e.pointerId)
  } catch {
    /* 已释放 */
  }
}

function onWheel(e) {
  if (!props.src) return
  const next = props.zoom + (e.deltaY > 0 ? -0.08 : 0.08)
  emit('update:zoom', Math.min(Math.max(Number(next.toFixed(2)), 1), 4))
}

function reset() {
  emit('update:zoom', 1)
  emit('update:offsetX', 0)
  emit('update:offsetY', 0)
}

function clamp(v) {
  return Number.isFinite(v) ? Math.min(Math.max(v, -50), 50) : 0
}
</script>

<style scoped>
.cropper {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

/* 圆形舞台：不用方形+遮罩，直接就是一个圆 */
.stage {
  position: relative;
  width: min(240px, 100%);
  aspect-ratio: 1 / 1;
  margin: 0 auto;
  border-radius: 50%;
  overflow: hidden;
  background: #080c16;
  border: 1px solid rgba(76, 201, 240, 0.35);
  cursor: grab;
  touch-action: none;
  user-select: none;
}

.stage.dragging {
  cursor: grabbing;
}

.photo {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: center;
  /* transform 来自 inline style，和 UserAvatar 的渲染保持同一套几何 */
  transform-origin: center;
  pointer-events: none;
  -webkit-user-drag: none;
}

.hint-empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.78rem;
  color: var(--text-muted);
}

/* 极淡的暗角，暗示"边缘会被裁掉" */
.shade {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  box-shadow: inset 0 0 24px rgba(0, 0, 0, 0.45);
  pointer-events: none;
}

.bar {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.zoom {
  flex: 1;
  height: 3px;
  appearance: none;
  -webkit-appearance: none;
  background: rgba(76, 201, 240, 0.25);
  border-radius: 999px;
  cursor: pointer;
}

.zoom::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--accent);
}

.zoom::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--accent);
  border: none;
}

.reset {
  flex-shrink: 0;
  padding: 0.16rem 0.55rem;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--text-muted);
  font-size: 0.7rem;
  font-family: inherit;
  cursor: pointer;
}

.reset:hover {
  border-color: var(--accent);
  color: var(--accent-soft);
}
</style>
