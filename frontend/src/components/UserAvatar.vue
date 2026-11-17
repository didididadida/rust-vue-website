<!--
  UserAvatar —— 站内统一头像

  逻辑描述：
    输入：username（必填）、avatarUrl（可选，来自个人资料）、size、link（是否包一层用户主页链接）
    输出：有头像图就显示图；没有就显示「默认头像」——冷色渐变底 + 用户名首字
    关键机制：
      - 头像图片来自登录资料（data/profiles/<user>.json 的 avatar 字段）
      - 列表页里作者的头像 URL 由后端随帖子一起下发（author_avatar），不再逐个请求
      - 默认头像的底色由用户名哈希决定，同一个人在任何页面颜色一致
    不变量：无论有没有图，渲染出来的尺寸与圆角都一样，列表不会因为缺头像而跳动
    边界：图片加载失败时回退到默认头像（@error），不会留下破图图标

  尺寸模式（关键）：
    - fit="cover"（默认）：填满容器，非正方形图会被裁成圆 —— 适合小头像
    - fit="contain"：**完整放入**容器，不裁切任何部分，四周留白 —— 适合个人主页大头像
      （头像原图常常不是正方形，cover 一定会裁掉边缘，用户看到的就是"头像被切了"）

  依据：
    经典问题：图片填充策略（object-fit 的语义）
    本处变体：头像原图比例不可控，而圆形裁切对比例极其敏感
    沿用结论：cover 保证不变形但会裁切；contain 保证不裁切但会出现留白
    偏离处：没有做手动裁剪框——直接用 contain + 同色底，留白看不出来
-->
<template>
  <component
    :is="link && username ? 'router-link' : 'span'"
    :to="link && username ? `/u/${encodeURIComponent(username)}` : undefined"
    class="avatar"
    :class="[`fit-${fit}`, { 'has-image': showImage, 'is-cropped': cropped }]"
    :style="[fallbackStyle, { width: size + 'px', height: size + 'px', fontSize: size * 0.42 + 'px' }]"
    :title="username ? '@' + username : ''"
  >
    <img
      v-if="showImage"
      :src="avatarUrl"
      :alt="username || '头像'"
      class="avatar-img"
      loading="lazy"
      :style="cropStyle"
      @error="broken = true"
    />
    <span v-else class="avatar-initial">{{ initial }}</span>
  </component>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { avatarStyle, avatarInitial } from '../time'

const props = defineProps({
  username: { type: String, default: '' },
  avatarUrl: { type: String, default: '' },
  size: { type: Number, default: 38 },
  link: { type: Boolean, default: true },
  /** 'cover'（填满，会裁切）| 'contain'（完整显示，留白） */
  fit: { type: String, default: 'cover' },
  /** 裁剪缩放倍数（1 = 刚好装下），与 AvatarCropper 的语义一致 */
  zoom: { type: Number, default: 1 },
  /** 裁剪偏移，相对容器宽高的百分比 */
  offsetX: { type: Number, default: 0 },
  offsetY: { type: Number, default: 0 }
})

const broken = ref(false)

const showImage = computed(() => !!props.avatarUrl && !broken.value)
const fallbackStyle = computed(() => avatarStyle(props.username))
const initial = computed(() => avatarInitial(props.username))

/**
 * 把裁剪参数翻译成 img 的 transform。
 *
 * 与 AvatarCropper 用同一套模型：
 *   translate(-50%,-50%) 把图片中心对到容器中心，
 *   再叠加用户的百分比偏移，最后 scale。
 * 封面模式（fit=cover）下图片本身是 object-fit: cover 铺满的，
 * 所以 scale 的基准就是那个"填满"的状态，缩放 1 = 原始裁剪。
 */
const cropStyle = computed(() => {
  const z = Number(props.zoom) || 1
  if (z === 1 && !props.offsetX && !props.offsetY) return {}
  return {
    transform: `scale(${z}) translate(${props.offsetX}%, ${props.offsetY}%)`
  }
})

/** 是否应用了裁剪参数：是的话渲染时改为"正方形铺满 + 圆角裁切"的路径 */
const cropped = computed(
  () => (Number(props.zoom) || 1) !== 1 || !!props.offsetX || !!props.offsetY
)

// 换了用户或换了头像地址时，重新允许加载
watch(
  () => [props.username, props.avatarUrl],
  () => {
    broken.value = false
  }
)
</script>

<style scoped>
.avatar {
  /* 关键：给内部绝对定位的图片建立自己的定位基准，
     否则图片会相对外层容器定位，超出圆形范围 */
  position: relative;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* 尺寸走 inline style（:style 里的 width/height），这里用变量兜底并锁死长宽比，
     避免父级 flex 布局把它拉伸成椭圆（曾导致头像只显示一半） */
  min-width: 0;
  aspect-ratio: 1 / 1;
  box-sizing: border-box;
  border-radius: 50%;
  overflow: hidden;
  color: #eaf2ff;
  font-weight: 600;
  font-family: var(--font-mono);
  text-decoration: none;
  border: 1px solid rgba(255, 255, 255, 0.1);
  user-select: none;
  align-self: flex-start;
}

.avatar-img {
  width: 100%;
  height: 100%;
  /* 默认填满并居中：小头像看起来最"饱满"，非正方形图会裁掉边缘 */
  object-fit: cover;
  object-position: center top;
  display: block;
}

/* contain 模式：完整显示整张图，不裁切任何部分。
   坑：如果只写 object-fit: contain，图片会缩到容器高度的 ~60%，
   看起来就像"头像变小了"。所以这里用绝对定位 + 让图片铺满容器，
   由 object-fit 在里面按原比例缩放到"刚好装下"。
   留白用半透明白底兜住，与头像框融为一体，不会像缺了一块。 */
.fit-contain {
  background-color: rgba(255, 255, 255, 0.06);
}

.fit-contain .avatar-img {
  /* 关键：绝对定位的基准必须是 .avatar 自己（它才是那个圆），
     如果基准落到外层容器上，图片的矩形会比圆大一圈，
     被 overflow: hidden 裁掉后，圆外会露出一圈图片的直角。 */
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  object-position: center;
  /* 避免透明 PNG 的边缘出现颜色带 */
  background-color: rgba(12, 16, 26, 0.5);
}

.avatar-initial {
  line-height: 1;
  opacity: 0.92;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
}

/* ---- 应用了裁剪参数时的渲染路径 ----
   把图片改成 object-fit: cover（填满正方形）+ transform 缩放平移，
   外层圆角负责裁切。这样"拖动看到的"和"最终渲染的"是同一套几何。 */
.avatar.is-cropped .avatar-img,
.fit-contain.is-cropped .avatar-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: center;
  background: transparent;
  transform-origin: center center;
  transition: transform 0.15s var(--ease);
}
</style>
