<!--
  Profile —— 个人主页（对齐 B 站「用户空间」的布局）

  逻辑描述：
    输入：GET /api/profile/{username} → { nickname, bio, avatar, banner, is_admin, joined_at,
          title, comment_count, recent_comments }
    输出：
      - 顶部**空间头图**（banner）：有图显示图，没图用主题渐变 + 星点
      - 压在头图下缘的**大头像 + 昵称/头衔/简介 + 资料卡**（B 站空间页的标志性叠放）
      - 下方「消息中心」：最近收到的评论与论坛回复，按时间合并
    关键机制：
      - 头图与内容区是同一个容器里的两层，用负 margin 让头像叠上去
      - 收到的信息来自两套系统（文章评论 + 论坛回复），前端按时间合并排序
      - 自己看自己时显示「编辑资料」，可以改昵称/简介/头像/头图
    不变量：无论有没有头图和头像，卡片高度与层级关系都不会塌
    边界：用户不存在时显示空态；无任何消息时显示占位而非空白

  依据：
    经典问题：渐进增强与缺省形态（progressive enhancement）
    本处变体：B 站空间页的视觉重心是"头图 + 叠在上面的头像"，而头图是可选资源
    沿用结论：把"没有头图"当成一种正常变体，用同尺寸的主题渐变占位，而不是塌掉
    偏离处：没有做粉丝数/关注数（本站没有社交关系），改用评论数与加入时间
-->
<template>
  <div class="profile-page anime-page" data-bg="profile">
    <div v-if="loading" class="loading">
      <el-skeleton :rows="8" animated />
    </div>

    <div v-else-if="!profile" class="empty">
      <el-empty description="用户不存在">
        <el-button type="primary" @click="$router.push('/blog')">去看看博客</el-button>
      </el-empty>
    </div>

    <template v-else>
      <!-- ============ 空间头图 + 资料卡 ============ -->
      <section class="space">
        <!-- 头图：自己看自己时整块可点，点开只编辑头图 -->
        <div
          class="banner"
          :class="{ 'is-empty': !profile.banner, editable: isSelf }"
          :style="bannerStyle"
          :role="isSelf ? 'button' : undefined"
          :tabindex="isSelf ? 0 : undefined"
          @click="isSelf && openEditor('banner')"
          @keydown.enter="isSelf && openEditor('banner')"
        >
          <img v-if="profile.banner" :src="profile.banner" :alt="`${profile.nickname} 的空间头图`" />
          <!-- 头图底部渐隐，让下面的文字可读 -->
          <span class="banner-fade"></span>
          <span v-if="isSelf" class="banner-edit">更换头图</span>
        </div>

        <!-- 资料卡（压住头图下缘） -->
        <div class="card">
          <div class="card-main">
            <!-- 左列：头像 + 下方的「编辑资料」 -->
            <div class="avatar-col">
              <!-- 头像：整块可点，点开只换头像。
                   尺寸是站点统一的常量（96px），不开放给用户自定义，
                   否则每个主页的头像大小都不一样，整体就不规范了。 -->
              <button
                v-if="isSelf"
                type="button"
                class="avatar-wrap editable"
                title="更换头像"
                @click="openEditor('avatar')"
              >
                <UserAvatar
                  :username="profile.username"
                  :avatar-url="profile.avatar"
                  :size="AVATAR_DISPLAY_SIZE"
                  :link="false"
                  :zoom="profile.avatar_zoom || 1"
                  :offset-x="profile.avatar_x || 0"
                  :offset-y="profile.avatar_y || 0"
                />
                <span class="avatar-ring"></span>
                <span class="avatar-mask">换头像</span>
              </button>
              <div v-else class="avatar-wrap">
                <UserAvatar
                  :username="profile.username"
                  :avatar-url="profile.avatar"
                  :size="AVATAR_DISPLAY_SIZE"
                  :link="false"
                  :zoom="profile.avatar_zoom || 1"
                  :offset-x="profile.avatar_x || 0"
                  :offset-y="profile.avatar_y || 0"
                />
                <span class="avatar-ring"></span>
              </div>

              <!-- 头像正下方：和「换头像」同一种胶囊按钮，点开编辑资料总览 -->
              <button
                v-if="isSelf"
                type="button"
                class="avatar-edit"
                title="编辑资料"
                @click="openEditor('profile')"
              >编辑资料</button>
            </div>

            <div class="who">
              <!-- 昵称：点开只改昵称 -->
              <div
                class="title-row"
                :class="{ editable: isSelf }"
                :role="isSelf ? 'button' : undefined"
                :tabindex="isSelf ? 0 : undefined"
                :title="isSelf ? '点击修改昵称' : ''"
                @click="isSelf && openEditor('nickname')"
                @keydown.enter="isSelf && openEditor('nickname')"
              >
                <h1 class="nickname">{{ profile.nickname }}</h1>
                <span class="rank">
                  <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor" stroke-width="1.6">
                    <circle cx="8" cy="8" r="6" />
                    <path d="M8 4.5v3.5l2 1.5" />
                  </svg>
                  {{ profile.title }}
                </span>
                <span v-if="profile.is_admin" class="rank admin">站长</span>
                <span v-if="isSelf" class="edit-pencil" v-html="iconSvg('image', 12)"></span>
              </div>
              <p class="username">@{{ profile.username }}</p>

              <!-- 简介：点开只改简介 -->
              <p
                v-if="profile.bio"
                class="bio"
                :class="{ editable: isSelf }"
                :title="isSelf ? '点击修改简介' : ''"
                :role="isSelf ? 'button' : undefined"
                :tabindex="isSelf ? 0 : undefined"
                @click="isSelf && openEditor('bio')"
                @keydown.enter="isSelf && openEditor('bio')"
              >{{ profile.bio }}</p>
              <p
                v-else
                class="bio bio-empty"
                :class="{ editable: isSelf }"
                :title="isSelf ? '点击添加简介' : ''"
                :role="isSelf ? 'button' : undefined"
                :tabindex="isSelf ? 0 : undefined"
                @click="isSelf && openEditor('bio')"
                @keydown.enter="isSelf && openEditor('bio')"
              >这个人很神秘，还没有写简介。{{ isSelf ? '点这里添加' : '' }}</p>
            </div>
          </div>

          <!-- 数据条：三项都可点，点「评论」「论坛回复」切换下方展示的内容。
               只有本人能把"我发过什么"展开来看，别人只看得到作品。 -->
          <div class="card-foot">
            <dl class="stats">
              <button type="button" class="stat" :class="{ active: tab === 'works' }" @click="tab = 'works'">
                <dt>作品</dt>
                <dd class="num">{{ works.length }}</dd>
              </button>
              <component
                :is="isSelf ? 'button' : 'div'"
                :type="isSelf ? 'button' : undefined"
                class="stat"
                :class="{ active: tab === 'comments', 'stat-plain': !isSelf }"
                @click="isSelf && switchTab('comments')"
              >
                <dt>评论</dt>
                <dd class="num">{{ isSelf ? commentCount : '—' }}</dd>
              </component>
              <component
                :is="isSelf ? 'button' : 'div'"
                :type="isSelf ? 'button' : undefined"
                class="stat"
                :class="{ active: tab === 'replies', 'stat-plain': !isSelf }"
                @click="isSelf && switchTab('replies')"
              >
                <dt>论坛回复</dt>
                <dd class="num">{{ isSelf ? replyCount : '—' }}</dd>
              </component>
            </dl>

            <div v-if="isSelf" class="card-actions">
              <router-link to="/forum/new" class="btn btn-ghost btn-sm">发新帖</router-link>
              <!-- 私信是唯一入口：系统通知就是「系统发给你的私信」，不再是独立页面 -->
              <!-- 图床仅管理员可见：编辑器已内建图片上传，普通用户不需要单独入口 -->
              <router-link v-if="auth.isAdmin" to="/images" class="btn btn-ghost btn-sm">我的图床</router-link>
            </div>
            <div v-else-if="isLoggedIn" class="card-actions">
            </div>
          </div>
        </div>
      </section>

      <!-- ============ 下方：作品 / 评论 / 论坛回复 三个视图 ============ -->
      <section class="section">
        <div class="section-head">
          <h2 class="section-title">{{ tabTitle }}</h2>
          <span class="section-count">{{ tabCount }} 条</span>
        </div>
        <p class="section-sub">{{ tabSub }}</p>

        <!-- 作品 -->
        <template v-if="tab === 'works'">
          <p v-if="loadingWorks" class="init-hint">加载中…</p>
          <p v-else-if="works.length === 0" class="empty-log">
            {{ isSelf ? '还没有发过帖子，去社区发第一帖吧' : '这个人还没有发布过作品' }}
          </p>
          <div v-else class="work-grid">
            <router-link
              v-for="w in works"
              :key="w.id"
              :to="`/forum/${w.id}`"
              class="work-card"
            >
              <div class="work-cover" :class="{ 'is-empty': !w.cover, 'is-video': !w.cover && w.has_video }">
                <img v-if="w.cover" :src="w.cover" :alt="w.title" loading="lazy" />
                <span v-else-if="w.has_video" class="work-icon work-video">
                  <span class="play-badge" v-html="iconSvg('play', 18)"></span>
                </span>
                <span v-else class="work-icon">{{ boardIcon(w.board_slug) }}</span>
              </div>
              <div class="work-body">
                <h3 class="work-title">{{ w.title }}</h3>
                <div class="work-meta">
                  <span class="work-board">{{ w.board_name }}</span>
                  <span class="work-stat">{{ compact(w.reply_count) }} 回复</span>
                  <span class="work-stat">{{ compact(w.like_count) }} 赞</span>
                </div>
              </div>
            </router-link>
          </div>
        </template>

        <!-- 评论（仅本人可见） -->
        <template v-else-if="tab === 'comments'">
          <p v-if="!isSelf" class="empty-log">只能查看自己的评论记录</p>
          <p v-else-if="loadingComments" class="init-hint">加载中…</p>
          <p v-else-if="comments.length === 0" class="empty-log">还没有发表过评论</p>
          <ul v-else class="log-list">
            <li v-for="c in comments" :key="c.id" class="log-item">
              <router-link :to="`/blog/${encodeURIComponent(c.slug)}`" class="log-article">
                {{ c.article_title }}
              </router-link>
              <p class="log-content">{{ c.content }}</p>
              <div class="log-meta">
                <span v-if="c.is_reply" class="log-tag">回复他人</span>
                <span v-if="!c.is_approved" class="log-tag pending">待审核</span>
                <time class="log-time">{{ relativeTime(c.created_at) }}</time>
              </div>
            </li>
          </ul>
        </template>

        <!-- 论坛回复（仅本人可见） -->
        <template v-else>
          <p v-if="!isSelf" class="empty-log">只能查看自己的回复记录</p>
          <p v-else-if="loadingReplies" class="init-hint">加载中…</p>
          <p v-else-if="replies.length === 0" class="empty-log">还没有在社区里回复过</p>
          <ul v-else class="log-list">
            <li v-for="r in replies" :key="r.id" class="log-item">
              <router-link :to="`/forum/${r.post_id}`" class="log-article">
                {{ r.post_title }}
              </router-link>
              <p class="log-content">{{ r.content }}</p>
              <div class="log-meta">
                <span class="log-board">{{ r.board_name }}</span>
                <span v-if="r.floor" class="log-tag">#{{ r.floor }} 楼</span>
                <span v-else class="log-tag">楼中楼</span>
                <span class="log-tag">{{ compact(r.like_count) }} 赞</span>
                <time class="log-time">{{ relativeTime(r.created_at) }}</time>
              </div>
            </li>
          </ul>
        </template>
      </section>
    </template>

    <!-- ============ 分区编辑弹层 ============
         每个区域点开只改一件事，弹窗里带实时预览，见下方 editorTitle/editorHint -->
    <Teleport to="body">
      <Transition name="sheet">
        <div v-if="editing" class="modal" @click.self="closeEditor">
          <div class="modal-card" role="dialog" :aria-label="editorTitle">
            <!-- 头部 -->
            <header class="modal-head">
              <span class="modal-icon" v-html="iconSvg(editorIcon, 18)"></span>
              <div class="modal-heading">
                <h2 class="modal-title">{{ editorTitle }}</h2>
                <p class="modal-sub">{{ editorHint }}</p>
              </div>
              <button type="button" class="modal-close" aria-label="关闭" @click="closeEditor">✕</button>
            </header>

            <!-- 总分览：从左列按钮进入时展示全部字段 -->
            <div v-if="editing === 'profile'" class="editor-body">
              <div class="profile-banner-preview">
                <div class="preview-banner" :class="{ 'is-empty': !form.banner }" :style="previewBannerStyle">
                  <span v-if="!form.banner" class="preview-empty">默认渐变</span>
                </div>
              </div>

              <label class="field-label">昵称</label>
              <input v-model.trim="form.nickname" class="input" maxlength="24" placeholder="输入昵称" />

              <label class="field-label">简介</label>
              <textarea
                v-model.trim="form.bio"
                class="input"
                rows="3"
                maxlength="200"
                placeholder="介绍一下自己吧"
              ></textarea>

              <label class="field-label">头像</label>
              <div
                class="drop-zone inline-zone"
                :class="{ 'is-over': dragOver === 'avatar', 'is-busy': uploading === 'avatar' }"
                @paste="onPaste('avatar', $event)"
                @dragover.prevent="dragOver = 'avatar'"
                @dragleave.prevent="dragOver = null"
                @drop.prevent="onDropImage('avatar', $event)"
                @click="pickFile('avatar')"
              >
                <UserAvatar
                  :username="profile.username"
                  :avatar-url="form.avatar"
                  :size="56"
                  :link="false"
                />
                <span class="inline-hint">
                  {{ uploading === 'avatar' ? '上传中…' : '点击选择 · 或粘贴截图' }}
                </span>
                <input
                  ref="avatarOverviewEl"
                  type="file"
                  accept="image/*"
                  class="hidden-file"
                  @change="onFilePick('avatar', $event)"
                />
              </div>

              <label class="field-label">空间头图</label>
              <div
                class="drop-zone inline-zone"
                :class="{ 'is-over': dragOver === 'banner', 'is-busy': uploading === 'banner' }"
                @paste="onPaste('banner', $event)"
                @dragover.prevent="dragOver = 'banner'"
                @dragleave.prevent="dragOver = null"
                @drop.prevent="onDropImage('banner', $event)"
                @click="pickFile('banner')"
              >
                <div class="inline-banner" :class="{ 'is-empty': !form.banner }" :style="previewBannerStyle"></div>
                <span class="inline-hint">
                  {{ uploading === 'banner' ? '上传中…' : '点击选择 · 或粘贴截图' }}
                </span>
                <input
                  ref="bannerOverviewEl"
                  type="file"
                  accept="image/*"
                  class="hidden-file"
                  @change="onFilePick('banner', $event)"
                />
              </div>
              <p class="hint">头像和头图都可以直接粘贴截图或拖入图片，不用再填链接</p>
            </div>

            <!-- 头图编辑器：同样支持粘贴 / 拖入 / 点选 -->
            <div v-else-if="editing === 'banner'" class="editor-body">
              <div
                class="drop-zone banner-zone"
                :class="{ 'is-over': dragOver === 'banner', 'is-busy': uploading === 'banner' }"
                @paste="onPaste('banner', $event)"
                @dragover.prevent="dragOver = 'banner'"
                @dragleave.prevent="dragOver = null"
                @drop.prevent="onDropImage('banner', $event)"
                @click="pickFile('banner')"
              >
                <div class="preview-banner" :class="{ 'is-empty': !form.banner }" :style="previewBannerStyle">
                  <span v-if="!form.banner" class="preview-empty">未设置头图时使用默认渐变</span>
                </div>
                <p class="drop-title">
                  {{ uploading === 'banner' ? '上传中…' : '点击选择图片，或直接粘贴截图' }}
                </p>
                <p class="drop-sub">也可以把图片拖到这里</p>
                <input
                  ref="bannerEditorEl"
                  type="file"
                  accept="image/*"
                  class="hidden-file"
                  @change="onFilePick('banner', $event)"
                />
              </div>

              <div class="drop-actions">
                <button type="button" class="btn-link" @click.stop="pickFile('banner')">选择图片</button>
                <button
                  v-if="form.banner"
                  type="button"
                  class="btn-link danger"
                  @click.stop="form.banner = ''"
                >
                  移除头图
                </button>
              </div>
              <p class="hint">建议 16:9 或更宽；直接粘贴截图即可</p>
            </div>

            <!-- 头像编辑器：先裁剪，再定显示大小 -->
            <div v-else-if="editing === 'avatar'" class="editor-body">
              <!-- 有图 → 进裁剪器；没图 → 显示上传区 -->
              <AvatarCropper
                v-if="form.avatar"
                v-model:zoom="form.avatar_zoom"
                v-model:offset-x="form.avatar_x"
                v-model:offset-y="form.avatar_y"
                :src="form.avatar"
              />

              <div
                v-else
                class="drop-zone"
                :class="{ 'is-over': dragOver === 'avatar', 'is-busy': uploading === 'avatar' }"
                @paste="onPaste('avatar', $event)"
                @dragover.prevent="dragOver = 'avatar'"
                @dragleave.prevent="dragOver = null"
                @drop.prevent="onDropImage('avatar', $event)"
                @click="pickFile('avatar')"
              >
                <div class="preview-avatar">
                  <UserAvatar
                    :username="profile.username"
                    :avatar-url="form.avatar"
                    :size="AVATAR_DISPLAY_SIZE"
                    :link="false"
                    fit="contain"
                  />
                </div>
                <p class="drop-title">
                  {{ uploading === 'avatar' ? '上传中…' : '点击选择图片，或直接粘贴截图' }}
                </p>
                <p class="drop-sub">也可以把图片拖到这里</p>
              </div>

              <input
                ref="avatarEditorEl"
                type="file"
                accept="image/*"
                class="hidden-file"
                @change="onFilePick('avatar', $event)"
              />

              <div class="drop-actions">
                <button type="button" class="btn-link" @click.stop="pickFile('avatar')">
                  {{ form.avatar ? '换一张图片' : '选择图片' }}
                </button>
                <button
                  v-if="form.avatar"
                  type="button"
                  class="btn-link danger"
                  @click.stop="form.avatar = ''"
                >
                  移除头像
                </button>
              </div>
              <p class="hint">
                {{ form.avatar ? '拖动图片调整位置，滚轮或滑块缩放，圆形区域就是最终效果' : '直接粘贴截图即可；留空则使用默认的冷色渐变头像' }}
              </p>
            </div>

            <!-- 昵称编辑器 -->
            <div v-else-if="editing === 'nickname'" class="editor-body">
              <input
                ref="focusEl"
                v-model.trim="form.nickname"
                class="input input-lg"
                maxlength="24"
                placeholder="输入昵称"
              />
              <p class="hint">最多 24 字 · 已用 {{ form.nickname.length }} 字</p>
            </div>

            <!-- 简介编辑器 -->
            <div v-else-if="editing === 'bio'" class="editor-body">
              <textarea
                ref="focusEl"
                v-model.trim="form.bio"
                class="input"
                rows="4"
                maxlength="200"
                placeholder="介绍一下自己吧"
              ></textarea>
              <p class="hint">最多 200 字 · 已用 {{ form.bio.length }} 字</p>
            </div>

            <p v-if="saveError" class="msg error">{{ saveError }}</p>

            <footer class="modal-actions">
              <button class="btn btn-ghost btn-sm" type="button" @click="closeEditor">取消</button>
              <button class="btn btn-sm" type="button" :disabled="saving" @click="save">
                {{ saving ? '保存中…' : '保存修改' }}
              </button>
            </footer>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup>
import { computed, nextTick, reactive, ref, watch, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import http from '../api'
import { auth } from '../auth'
import { formatDate } from '../labels'
import { relativeTime, extractUrl, compact } from '../time'
import { iconSvg } from '../emoji'
import { isLoggedIn } from '../auth'
import UserAvatar from '../components/UserAvatar.vue'
import AvatarCropper from '../components/AvatarCropper.vue'

const route = useRoute()
const profile = ref(null)
const loading = ref(true)
const saving = ref(false)
const saveError = ref('')
const focusEl = ref(null)

/**
 * 当前打开的编辑器：null 表示关闭。
 * 'banner' | 'avatar' | 'nickname' | 'bio'
 *
 * 为什么拆成"点哪改哪"：
 *   原来一个弹窗塞了 4 个字段，改昵称也得把头像链接看一遍；
 *   而资料卡本身就是"头像/昵称/简介/头图"四块的视觉集合，直接把可点区域映射到字段，
 *   用户的路径最短、也没有"这弹窗到底改哪个"的疑问。
 *
 * 依据：
 *   经典问题：就地编辑（in-place editing）——让可点区域与其对应的数据一一对应
 *   本处变体：表单字段仍是同一套，只是入口和弹窗按区域拆分
 *   沿用结论：保存接口不变（PUT /api/profile 始终提交整份资料），
 *             前端只控制"展示哪一块的编辑器"
 *   偏离处：弹窗里的改动不会实时写回页面，仍需点「保存修改」确认
 */
const editing = ref(null)

const EDITOR_META = {
  banner: { title: '更换头图', hint: '出现在个人主页顶部的宽幅大图', icon: 'image' },
  avatar: { title: '更换头像', hint: '显示在资料卡左上角的圆形头像', icon: 'smile' },
  nickname: { title: '修改昵称', hint: '别人在评论、帖子里看到的名字', icon: 'crown' },
  bio: { title: '修改简介', hint: '一句话介绍自己', icon: 'comment' },
  profile: { title: '编辑资料', hint: '昵称、简介、头像、头图都可以在这里一次改完', icon: 'crown' }
}

const editorTitle = computed(() => EDITOR_META[editing.value]?.title || '')
const editorHint = computed(() => EDITOR_META[editing.value]?.hint || '')
const editorIcon = computed(() => EDITOR_META[editing.value]?.icon || 'smile')

/** 表单初值：打开弹窗时才填，避免沿用上一次编辑的残留 */
const form = reactive({
  nickname: '',
  bio: '',
  avatar: '',
  banner: '',
  avatar_zoom: 1,
  avatar_x: 0,
  avatar_y: 0
})

/** 图片上传中状态：'avatar' | 'banner' | null */
const uploading = ref(null)
/** 拖拽悬停高亮 */
const dragOver = ref(null)

function openEditor(kind) {
  if (!profile.value) return
  form.nickname = profile.value.nickname
  form.bio = profile.value.bio
  form.avatar = profile.value.avatar
  form.banner = profile.value.banner || ''
  form.avatar_zoom = profile.value.avatar_zoom || 1
  form.avatar_x = profile.value.avatar_x || 0
  form.avatar_y = profile.value.avatar_y || 0
  saveError.value = ''
  editing.value = kind
  // 文本类编辑器自动聚焦
  if (kind === 'nickname' || kind === 'bio') {
    nextTick(() => focusEl.value?.focus())
  }
}

function closeEditor() {
  editing.value = null
  saveError.value = ''
}

const target = computed(() => route.params.username || auth.username)
const isSelf = computed(() => profile.value && auth.username === profile.value.username)

/** 头图：有图用图，没图交给 CSS 渐变（见 .banner.is-empty） */
const bannerStyle = computed(() => {
  const url = profile.value?.banner
  return url ? { backgroundImage: `url("${url}")` } : {}
})

/** 个人主页头像的显示尺寸 —— 站点统一常量，不开放给用户自定义 */
const AVATAR_DISPLAY_SIZE = 96

/** 编辑器里的头图实时预览 */
const previewBannerStyle = computed(() => {
  const url = form.banner
  return url ? { backgroundImage: `url("${url}")` } : {}
})


/** 作品中心：该用户在社区发过的帖子（一次拉取，取前 12 个） */
const works = ref([])
const loadingWorks = ref(true)

/** 下方展示哪一块：作品 / 评论 / 论坛回复 */
const tab = ref('works')

/** 「评论」：该用户发表过的文章评论 */
const comments = ref([])
const loadingComments = ref(false)
let commentsLoaded = false

/** 「论坛回复」：该用户发表过的论坛回复 */
const replies = ref([])
const loadingReplies = ref(false)
let repliesLoaded = false

const commentCount = computed(() => comments.value.length || profile.value?.comment_count || 0)
const replyCount = computed(() => replies.value.length)

const tabTitle = computed(() => {
  if (tab.value === 'comments') return '评论'
  if (tab.value === 'replies') return '论坛回复'
  return '作品中心'
})

const tabSub = computed(() => {
  if (tab.value === 'comments') return 'TA 在文章下发表过的评论'
  if (tab.value === 'replies') return 'TA 在社区里发表过的回复'
  return 'TA 在社区里发过的帖子'
})

const tabCount = computed(() => {
  if (tab.value === 'comments') return comments.value.length
  if (tab.value === 'replies') return replies.value.length
  return works.value.length
})

/**
 * 切换下方视图。
 * 只对本人开放：别人的评论/回复记录属于个人历史，不对外展示。
 * 评论/回复是懒加载：首次点开才请求，避免主页一进来就发三个请求。
 */
async function switchTab(next) {
  if (!isSelf.value) return
  tab.value = next
  if (next === 'comments' && !commentsLoaded) await loadComments()
  if (next === 'replies' && !repliesLoaded) await loadReplies()
}

async function loadComments() {
  if (!profile.value) return
  loadingComments.value = true
  try {
    const data = await http.get(`/api/users/${encodeURIComponent(profile.value.username)}/comments`)
    comments.value = data.items || []
    commentsLoaded = true
  } catch (e) {
    console.error('加载评论失败:', e)
    comments.value = []
  } finally {
    loadingComments.value = false
  }
}

async function loadReplies() {
  if (!profile.value) return
  loadingReplies.value = true
  try {
    const data = await http.get(`/api/users/${encodeURIComponent(profile.value.username)}/replies`)
    replies.value = data.items || []
    repliesLoaded = true
  } catch (e) {
    console.error('加载论坛回复失败:', e)
    replies.value = []
  } finally {
    loadingReplies.value = false
  }
}

/** 板块图标（与社区页保持一致） */
function boardIcon(slug) {
  const map = { lounge: '💬', tech: '⚙️', study: '📚', creation: '🎨', help: '🆘' }
  return map[slug] || '📌'
}

async function loadWorks(username) {
  loadingWorks.value = true
  try {
    const data = await http.get('/api/forum/posts', {
      params: { author: username, limit: 12 }
    })
    works.value = data.items || []
  } catch (e) {
    console.error('加载作品失败:', e)
    works.value = []
  } finally {
    loadingWorks.value = false
  }
}

async function load() {
  if (!target.value) {
    loading.value = false
    profile.value = null
    return
  }
  loading.value = true
  try {
    profile.value = await http.get(`/api/profile/${encodeURIComponent(target.value)}`)
    form.nickname = profile.value.nickname
    form.bio = profile.value.bio
    form.avatar = profile.value.avatar
    form.banner = profile.value.banner || ''
      form.avatar_zoom = profile.value.avatar_zoom || 1
    form.avatar_x = profile.value.avatar_x || 0
    form.avatar_y = profile.value.avatar_y || 0
    loadWorks(profile.value.username)
    // 评论/论坛回复是本人的历史记录：只有看自己主页时才拉
    if (isSelf.value) {
      loadComments()
      loadReplies()
    }
  } catch (e) {
    console.error('加载个人主页失败:', e)
    profile.value = null
  } finally {
    loading.value = false
  }
}

/**
 * 粘贴处理：图片直接上传，文本按链接解析。
 *
 * 输入 → 输出：paste 事件 → 剪贴板里的图片文件则上传；否则走原来的 Markdown/URL 提取
 * 关键机制：先判断有没有图片文件，有就上传并拦下默认行为；
 *           纯文本才交给 extractUrl 收敛成裸链接
 * 不变量：粘贴图片时输入框绝不会被塞进一串 base64 或乱码文本
 * 边界：截图工具给的 PNG 走这条路；从网页复制的图片若只有 URL 而没有文件，
 *       会退化成文本分支，仍能拿到链接
 */
function onPaste(field, event) {
  const files = pickImageFiles(event.clipboardData)
  if (files.length) {
    event.preventDefault()
    uploadImage(field, files[0])
    return
  }

  const text = event.clipboardData?.getData('text') || ''
  const url = extractUrl(text)
  if (!url) return
  event.preventDefault()
  form[field] = url
}

/**
 * 从剪贴板 / 拖拽里取出图片文件。
 *
 * 输入 → 输出：DataTransfer → 图片 File 数组
 * 关键机制：先看 items（粘贴截图时文件在这里），再看 files；只保留 image/*
 * 不变量：非图片内容一律返回空数组，不会误把文本当成图片
 * 边界：Word/Excel 等富文本同时带图带文本时，只取图，文本由调用方放行
 */
function pickImageFiles(dt) {
  if (!dt) return []
  const out = []
  if (dt.items && dt.items.length) {
    for (const item of dt.items) {
      if (item.kind === 'file' && item.type.startsWith('image/')) {
        const f = item.getAsFile()
        if (f) out.push(f)
      }
    }
  }
  if (!out.length && dt.files) {
    for (const f of dt.files) {
      if (f.type.startsWith('image/')) out.push(f)
    }
  }
  return out
}

/**
 * 上传图片并把返回的地址写进对应字段。
 *
 * 输入 → 输出：File → 后端 /api/upload → form[field] = /static/uploads/xxx
 * 关键机制：复用发表格/编辑器的同一个上传端点，前端只关心拿回 url
 * 不变量：成功时 form[field] 一定是一个可直接用于 <img src> 的站内路径
 * 边界：多选时只用第一张（头像是单值字段）；失败保留原值并提示，不静默清空
 *
 * 依据：
 *   经典问题：用户输入成本最小化（直接操作优于转义表示）
 *   本处变体：头像是单值图片字段，原方案要求用户先拿到外链再填 URL
 *   沿用结论：把「先得到资源、再填引用」两步合并为「粘贴资源」一步
 *   偏离处：不做裁剪与压缩，落库仍是原图；需要裁切时应另加上传前处理
 */
async function uploadImage(field, file) {
  if (!file) return
  if (!file.type.startsWith('image/')) {
    saveError.value = '请粘贴或拖入图片文件'
    return
  }
  uploading.value = field
  saveError.value = ''
  try {
    const body = new FormData()
    body.append('file', file)
    const res = await http.post('/api/upload', body, {
      headers: { 'Content-Type': 'multipart/form-data' }
    })
    form[field] = res.url
  } catch (e) {
    saveError.value = e.message || '图片上传失败'
  } finally {
    uploading.value = null
  }
}

/** 选择本地文件后上传 */
function onFilePick(field, event) {
  const file = event.target.files?.[0]
  event.target.value = ''
  uploadImage(field, file)
}

/** 拖入图片 */
function onDropImage(field, event) {
  dragOver.value = null
  const files = pickImageFiles(event.dataTransfer)
  if (files.length) uploadImage(field, files[0])
  else saveError.value = '请拖入图片文件'
}

/**
 * 文件选择框的模板 ref。
 * 每个投放区各自一个：概览弹窗与单字段编辑器是不同的 v-if 分支，
 * 分开持有能避免"同一 ref 名被后挂载的元素覆盖"。
 */
const avatarOverviewEl = ref(null)
const bannerOverviewEl = ref(null)
const avatarEditorEl = ref(null)
const bannerEditorEl = ref(null)

/**
 * 打开系统文件选择框。
 * 优先用当前可见的那个：概览弹窗时编辑器不存在，反之亦然。
 */
function pickFile(field) {
  const el =
    field === 'avatar'
      ? avatarEditorEl.value || avatarOverviewEl.value
      : bannerEditorEl.value || bannerOverviewEl.value
  el?.click()
}

async function save() {
  saving.value = true
  saveError.value = ''
  try {
    await http.put('/api/profile', {
      nickname: form.nickname,
      bio: form.bio,
      avatar: form.avatar,
      banner: form.banner,
      avatar_zoom: form.avatar_zoom || 1,
      avatar_x: form.avatar_x || 0,
      avatar_y: form.avatar_y || 0
    })
    editing.value = null
    await load()
  } catch (e) {
    saveError.value = e.message || '保存失败'
  } finally {
    saving.value = false
  }
}

watch(() => route.params.username, load)
onMounted(load)
defineExpose({ load })
</script>

<style scoped>
.profile-page {
  max-width: 880px;
  margin: 0 auto;
}

/* ==================== 空间头图 ==================== */
.space {
  position: relative;
  margin-bottom: 2.5rem;
}

.banner {
  position: relative;
  width: 100%;
  aspect-ratio: 21 / 6;
  min-height: 150px;
  max-height: 260px;
  border-radius: var(--radius-lg);
  overflow: hidden;
  background-color: #0d1322;
  background-size: cover;
  background-position: center;
  border: 1px solid rgba(255, 255, 255, 0.09);
}

.banner img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

/* 没有头图：冷色渐变 + 一点星尘，和站点深空主题一致 */
.banner.is-empty {
  background-image:
    radial-gradient(1px 1px at 12% 30%, rgba(255, 255, 255, 0.5), transparent),
    radial-gradient(1px 1px at 38% 68%, rgba(255, 255, 255, 0.35), transparent),
    radial-gradient(1.5px 1.5px at 72% 24%, rgba(255, 255, 255, 0.45), transparent),
    radial-gradient(1px 1px at 88% 62%, rgba(255, 255, 255, 0.3), transparent),
    linear-gradient(120deg, #10243a 0%, #15203a 45%, #1b1a34 100%);
}

/* 底部渐隐：让压在头图上的资料卡文字永远可读 */
.banner-fade {
  position: absolute;
  inset: auto 0 0 0;
  height: 65%;
  background: linear-gradient(180deg, transparent, rgba(6, 9, 18, 0.92));
  pointer-events: none;
}

.banner-edit {
  position: absolute;
  top: 0.75rem;
  right: 0.75rem;
  z-index: 2;
  padding: 0.32rem 0.85rem;
  font-size: 0.78rem;
  font-family: inherit;
  color: var(--text-secondary);
  background: rgba(10, 14, 24, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 999px;
  pointer-events: none;
  backdrop-filter: blur(8px);
  opacity: 0;
  transition: opacity 0.2s var(--ease);
}

/* 头图整块可点：hover 才浮现"更换头图"，不喧宾夺主 */
.banner.editable {
  cursor: pointer;
}

.banner.editable:hover .banner-edit,
.banner.editable:focus-visible .banner-edit {
  opacity: 1;
}

.banner.editable:hover {
  border-color: rgba(255, 255, 255, 0.2);
}

.banner.editable:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

/* ==================== 资料卡（叠在头图上） ==================== */
.card {
  position: relative;
  /* 提到头图之上：否则头像压在头图上时，头图的矩形边缘会从圆的缺口处透出来 */
  z-index: 2;
  /* 头图下沉改为 -36px（原 -46px）：露出的头图更多，头像看起来更"往里收" */
  margin: -36px 1.25rem 0;
  padding: 1.35rem 1.5rem 1.1rem;
  border-radius: var(--radius-lg);
  background: rgba(14, 18, 30, 0.86);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: 0 14px 44px rgba(0, 0, 0, 0.4);
}

.card-main {
  display: flex;
  gap: 1.25rem;
  align-items: flex-start;
}

/* 左列：头像 + 「编辑资料」按钮。
   整列往下挪一点（padding-top），让头像不再骑在头图边界上——
   之前给 -58px 的负边距让头像压进头图一大截，视觉上"贴边"很难看。
   align-self 防止本列被 flex 拉伸成资料卡高度，否则头像会被拽成椭圆。 */
.avatar-col {
  flex-shrink: 0;
  align-self: flex-start;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.55rem;
  padding-top: 1.15rem;
}

.avatar-wrap {
  position: relative;
  flex-shrink: 0;
  /* 负边距从 -58px 收到 -30px：头像只让上沿压住卡边一点点，
     整颗头像和「编辑资料」按钮都落在资料卡内部 */
  margin-top: -30px;
  width: 96px;
  height: 96px;
  padding: 0;
  border: none;
  border-radius: 50%;
  /* 深色托底：头图/背景从圆外透进来时会被这层挡住，圆边永远清晰 */
  background: #0e1320;
  cursor: default;
  /* 这个 div/button 自己也要是圆并裁切子元素。
     之前只给内部 .avatar 设了 border-radius，外层的方形容器在
     无边框的白色头像图上会露出一圈直角（看起来就是方的）。 */
  overflow: hidden;
}

/* 自我主页时它是 <button>：button 默认 overflow 是 visible，会覆盖上面的 hidden，
   所以再显式声明一次；外圈用 box-shadow 画描边而不是 border，避免撑开尺寸 */
.avatar-wrap.editable {
  cursor: pointer;
  overflow: hidden;
  /* 圆环描边用 shadow 画，不占尺寸；再加一层深色托底，
     让头像压在头图上时也有一圈清晰的圆边 */
  box-shadow: 0 0 0 2px rgba(12, 16, 26, 0.9), 0 0 0 3px rgba(255, 255, 255, 0.12);
  background: #0e1320;
}

/* 可编辑态：hover 出现"换头像"遮罩（cursor 已在上面的规则里设置） */

.avatar-mask {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-size: 0.72rem;
  color: #fff;
  background: rgba(6, 10, 20, 0.6);
  opacity: 0;
  transition: opacity 0.2s var(--ease);
  z-index: 2;
}

.avatar-wrap.editable:hover .avatar-mask,
.avatar-wrap.editable:focus-visible .avatar-mask {
  opacity: 1;
}

/* 头像本体必须是完整的圆：外层裁切 + 内层圆角双保险 */
.avatar-wrap :deep(.avatar) {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  overflow: hidden;
  display: block;
}

/* 「编辑资料」：与头像 hover 遮罩同一种胶囊样式，常驻显示在头像正下方 */
.avatar-edit {
  padding: 0.26rem 0.8rem;
  font-size: 0.72rem;
  font-family: inherit;
  color: var(--text-secondary);
  background: rgba(10, 14, 24, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 999px;
  cursor: pointer;
  white-space: nowrap;
  backdrop-filter: blur(8px);
  transition: all 0.2s var(--ease);
}

.avatar-edit:hover {
  color: #fff;
  border-color: rgba(255, 255, 255, 0.32);
  background: rgba(20, 26, 40, 0.8);
}

.avatar-edit:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

/* 卡片里的字段标签（总分览编辑器用） */
.field-label {
  display: block;
  font-size: 0.74rem;
  color: var(--text-secondary);
  margin-top: 0.35rem;
}

.profile-banner-preview {
  margin-bottom: 0.2rem;
}

/* 可编辑文字区域：hover 时轻微高亮，提示可点 */
.title-row.editable,
.bio.editable {
  cursor: pointer;
  border-radius: var(--radius-sm);
  transition: background 0.18s var(--ease);
}

.title-row.editable:hover,
.bio.editable:hover {
  background: rgba(255, 255, 255, 0.05);
}

.edit-pencil {
  display: inline-flex;
  opacity: 0;
  color: var(--accent-soft);
  transition: opacity 0.18s var(--ease);
}

.title-row.editable:hover .edit-pencil {
  opacity: 0.8;
}

/* 头像外一圈缓慢旋转的冷光弧 */
.avatar-ring {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: conic-gradient(
    from 0deg,
    transparent 0deg,
    var(--accent) 40deg,
    transparent 120deg,
    transparent 360deg
  );
  opacity: 0.5;
  animation: ringSpin 6s linear infinite;
  pointer-events: none;
  -webkit-mask: radial-gradient(farthest-side, transparent calc(100% - 2px), #000 calc(100% - 2px));
  mask: radial-gradient(farthest-side, transparent calc(100% - 2px), #000 calc(100% - 2px));
}

@keyframes ringSpin {
  to {
    transform: rotate(360deg);
  }
}

.who {
  flex: 1;
  min-width: 0;
  padding-top: 0.2rem;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
  margin-bottom: 0.25rem;
}

.nickname {
  font-family: var(--font-serif);
  font-size: 1.5rem;
  font-weight: 600;
  color: #eef4ff;
  line-height: 1.3;
}

.rank {
  display: inline-flex;
  align-items: center;
  gap: 0.28rem;
  font-family: var(--font-mono);
  font-size: 0.66rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: var(--accent);
  padding: 0.14rem 0.52rem;
  border: 1px solid rgba(76, 201, 240, 0.32);
  border-radius: var(--radius-full);
  background: rgba(76, 201, 240, 0.08);
}

.rank.admin {
  color: var(--nebula-soft);
  border-color: rgba(123, 108, 246, 0.34);
  background: rgba(123, 108, 246, 0.1);
}

.username {
  font-family: var(--font-mono);
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-bottom: 0.7rem;
}

.bio {
  font-size: 0.9rem;
  line-height: 1.75;
  color: var(--text-secondary);
  white-space: pre-wrap;
}

.bio-empty {
  color: var(--text-muted);
  font-style: italic;
}

/* 底部数据条 */
.card-foot {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
  margin-top: 1rem;
  padding-top: 0.85rem;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.stats {
  display: flex;
  gap: 0.5rem;
  margin: 0;
}

/* 数据项做成按钮：有明显的可点感，选中项有冷光下划线 */
.stat {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.1rem;
  padding: 0.3rem 0.75rem;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  font-family: inherit;
  cursor: pointer;
  transition: all 0.2s var(--ease);
}

.stat:hover {
  background: rgba(255, 255, 255, 0.05);
}

.stat.active {
  background: rgba(76, 201, 240, 0.1);
  border-color: rgba(76, 201, 240, 0.28);
}

.stat dt {
  font-size: 0.7rem;
  color: var(--text-muted);
  letter-spacing: 0.08em;
}

.stat.active dt {
  color: var(--accent-soft);
}

/* 别人的主页：数字不展示，也不给按钮手感 */
.stat.stat-plain {
  cursor: default;
  opacity: 0.7;
}

.stat.stat-plain:hover {
  background: transparent;
}

.stat dd {
  font-size: 0.86rem;
  color: var(--text-primary);
  margin: 0;
}

.stat .num {
  font-family: var(--font-mono);
  font-size: 1.05rem;
  color: var(--accent-soft);
}

.card-actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

/* 次级按钮：细边无填充 */
.btn-ghost {
  background: transparent;
  border-color: rgba(255, 255, 255, 0.16);
  color: var(--text-secondary);
}

.btn-ghost:hover {
  border-color: rgba(255, 255, 255, 0.3);
  color: #fff;
}

/* ==================== 消息中心 ==================== */
.section {
  margin-top: 1.5rem;
}

.section-title {
  font-family: var(--font-serif);
  font-size: 1.2rem;
  font-weight: 600;
  color: #eef4ff;
  letter-spacing: 0.05em;
  padding-left: 0.85rem;
  position: relative;
}

.section-title::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0.24em;
  bottom: 0.24em;
  width: 2px;
  border-radius: var(--radius-full);
  background: linear-gradient(180deg, var(--accent), transparent);
  box-shadow: 0 0 10px rgba(76, 201, 240, 0.55);
}

.section-sub {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin: 0.3rem 0 1.1rem 0.85rem;
}

.empty-log {
  padding: 2.5rem 0;
  text-align: center;
  color: var(--text-muted);
  font-size: 0.86rem;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
}

/* ---------- 作品中心 ---------- */
.section-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 1rem;
}

.section-count {
  font-family: var(--font-mono);
  font-size: 0.76rem;
  color: var(--text-muted);
}

.work-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 0.75rem;
}

.work-card {
  display: flex;
  flex-direction: column;
  border-radius: var(--radius-md);
  overflow: hidden;
  background: rgba(16, 20, 32, 0.42);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: inherit;
  text-decoration: none;
  transition: border-color 0.22s var(--ease), transform 0.22s var(--ease-out),
    background 0.22s var(--ease);
}

.work-card:hover {
  border-color: rgba(255, 255, 255, 0.2);
  background: rgba(24, 30, 46, 0.55);
  transform: translateY(-2px);
}

.work-cover {
  aspect-ratio: 16 / 9;
  overflow: hidden;
  background: var(--bg-overlay);
  display: flex;
  align-items: center;
  justify-content: center;
}

.work-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/* 无封面：冷色渐变 + 板块图标，避免出现空白灰块 */
.work-cover.is-empty {
  background: linear-gradient(135deg, rgba(76, 201, 240, 0.1), rgba(123, 108, 246, 0.08));
}

.work-icon {
  font-size: 1.5rem;
  opacity: 0.55;
}

/* 视频作品占位：深色底 + 播放按钮 */
.work-cover.is-video {
  background: linear-gradient(135deg, #1b2436, #10161f);
}

.work-video {
  opacity: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.work-video .play-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.14);
  border: 1.5px solid rgba(255, 255, 255, 0.5);
  color: #fff;
}

.work-video .play-badge :deep(svg) {
  margin-left: 2px;
}

.work-body {
  padding: 0.65rem 0.8rem 0.75rem;
}

.work-title {
  font-family: var(--font-serif);
  font-size: 0.9rem;
  font-weight: 600;
  color: #e8f0fc;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 0.4rem;
}

.work-card:hover .work-title {
  color: var(--accent-soft);
}

.work-meta {
  display: flex;
  gap: 0.7rem;
  font-size: 0.7rem;
  color: var(--text-muted);
}

.work-board {
  color: var(--accent-soft);
}

.work-stat {
  font-family: var(--font-mono);
}

/* ---------- 评论 / 论坛回复列表 ---------- */
.log-list {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}

.log-item {
  padding: 0.85rem 1rem;
  border-radius: var(--radius-md);
  background: rgba(16, 20, 32, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.08);
  transition: border-color 0.2s var(--ease), background 0.2s var(--ease);
}

.log-item:hover {
  border-color: rgba(255, 255, 255, 0.18);
  background: rgba(24, 30, 46, 0.5);
}

.log-article {
  display: block;
  font-family: var(--font-serif);
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--accent-soft);
  text-decoration: none;
  margin-bottom: 0.3rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.log-article:hover {
  text-decoration: underline;
}

.log-content {
  font-size: 0.86rem;
  line-height: 1.7;
  color: var(--text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 0.4rem;
}

.log-meta {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.log-board {
  font-size: 0.7rem;
  color: var(--accent-soft);
}

.log-tag {
  font-size: 0.68rem;
  color: var(--text-muted);
  padding: 0.05rem 0.4rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  font-family: var(--font-mono);
}

.log-tag.pending {
  color: var(--warning);
  border-color: rgba(217, 178, 107, 0.35);
}

.log-time {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: 0.7rem;
  color: var(--text-muted);
}

.init-hint {
  font-size: 0.84rem;
  color: var(--text-muted);
  padding: 1.5rem 0;
  text-align: center;
}

/* ==================== 分区编辑弹层 ==================== */
.modal {
  position: fixed;
  inset: 0;
  z-index: 400;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.25rem;
  background: rgba(3, 5, 11, 0.72);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
}

.modal-card {
  width: 100%;
  max-width: 460px;
  border-radius: 18px;
  padding: 1.5rem;
  background: linear-gradient(165deg, rgba(22, 28, 46, 0.98), rgba(11, 15, 27, 0.99));
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.6), 0 0 0 1px rgba(76, 201, 240, 0.06);
  /* 顶部一条冷光虚线，和站点其它卡片的视觉语言一致 */
  position: relative;
  overflow: hidden;
}

.modal-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 12%;
  right: 12%;
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--accent), transparent);
  opacity: 0.7;
}

.modal-head {
  display: flex;
  align-items: flex-start;
  gap: 0.7rem;
  margin-bottom: 1.1rem;
}

.modal-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  flex-shrink: 0;
  border-radius: 10px;
  color: var(--accent-soft);
  background: rgba(76, 201, 240, 0.12);
  border: 1px solid rgba(76, 201, 240, 0.24);
}

.modal-heading {
  flex: 1;
  min-width: 0;
}

.modal-title {
  font-family: var(--font-serif);
  font-size: 1.12rem;
  font-weight: 600;
  color: #eef4ff;
  margin-bottom: 0.15rem;
}

.modal-sub {
  font-size: 0.76rem;
  color: var(--text-muted);
}

.modal-close {
  flex-shrink: 0;
  width: 1.9rem;
  height: 1.9rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 8px;
  color: var(--text-muted);
  font-size: 0.95rem;
  cursor: pointer;
  transition: all 0.18s var(--ease);
}

.modal-close:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.editor-body {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}

/* 头图预览：与主页同一个比例 */
.preview-banner {
  width: 100%;
  aspect-ratio: 21 / 6;
  min-height: 76px;
  border-radius: 10px;
  background-color: #0d1322;
  background-size: cover;
  background-position: center;
  border: 1px solid rgba(255, 255, 255, 0.1);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.preview-banner.is-empty {
  background-image: linear-gradient(120deg, #10243a 0%, #15203a 45%, #1b1a34 100%);
}

.preview-empty {
  font-size: 0.74rem;
  color: var(--text-muted);
}

.preview-avatar {
  display: flex;
  justify-content: center;
  padding: 0.35rem 0 0.55rem;
}

/* ---------- 图片投放区：粘贴 / 拖入 / 点选 ---------- */
.drop-zone {
  padding: 0.75rem;
  border: 1px dashed rgba(255, 255, 255, 0.18);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.02);
  cursor: pointer;
  transition: border-color 0.2s var(--ease), background 0.2s var(--ease);
  text-align: center;
}

.drop-zone:hover {
  border-color: rgba(76, 201, 240, 0.4);
  background: rgba(76, 201, 240, 0.04);
}

/* 拖拽悬停：整块亮起，明确告诉用户"松手即上传" */
.drop-zone.is-over {
  border-color: var(--accent);
  background: rgba(76, 201, 240, 0.1);
  box-shadow: inset 0 0 0 1px rgba(76, 201, 240, 0.3);
}

.drop-zone.is-busy {
  cursor: progress;
  opacity: 0.75;
}

.drop-title {
  margin-top: 0.5rem;
  font-size: 0.84rem;
  color: var(--text-secondary);
}

.drop-sub {
  font-size: 0.74rem;
  color: var(--text-muted);
  margin-top: 0.15rem;
}

.drop-actions {
  display: flex;
  gap: 0.9rem;
  justify-content: center;
  margin-top: 0.7rem;
}

.btn-link {
  background: none;
  border: none;
  padding: 0.2rem 0;
  font-family: inherit;
  font-size: 0.8rem;
  color: var(--accent-soft);
  cursor: pointer;
  transition: color 0.2s var(--ease);
}

.btn-link:hover {
  color: var(--accent);
}

.btn-link.danger {
  color: var(--text-muted);
}

.btn-link.danger:hover {
  color: var(--danger);
}

.hidden-file {
  display: none;
}

/* 「编辑资料」总览里的紧凑投放区：左边预览，右边提示 */
.inline-zone {
  display: flex;
  align-items: center;
  gap: 0.85rem;
  text-align: left;
  padding: 0.55rem 0.75rem;
}

.inline-banner {
  width: 120px;
  height: 52px;
  flex-shrink: 0;
  border-radius: 8px;
  background-color: #0d1322;
  background-size: cover;
  background-position: center;
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.inline-banner.is-empty {
  background-image: linear-gradient(120deg, #10243a 0%, #15203a 45%, #1b1a34 100%);
}

.inline-hint {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.input {
  width: 100%;
  padding: 0.62rem 0.85rem;
  background: #060a14;
  border: 1px solid var(--border);
  border-radius: 10px;
  color: var(--text-primary);
  font-size: 0.9rem;
  font-family: inherit;
  line-height: 1.65;
  resize: vertical;
}

.input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(76, 201, 240, 0.12);
}

/* 昵称输入框大一号，因为它本身就是"大字" */
.input-lg {
  font-family: var(--font-serif);
  font-size: 1.1rem;
  font-weight: 600;
}

.hint {
  font-size: 0.72rem;
  color: var(--text-muted);
}



.msg.error {
  font-size: 0.78rem;
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.08);
  border: 1px solid rgba(248, 113, 113, 0.22);
  padding: 0.42rem 0.65rem;
  border-radius: 8px;
  margin-top: 0.7rem;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.6rem;
  margin-top: 1.25rem;
}

/* 弹层进出场：上浮 + 淡入，比直接闪现自然 */
.sheet-enter-active,
.sheet-leave-active {
  transition: opacity 0.22s var(--ease);
}

.sheet-enter-active .modal-card,
.sheet-leave-active .modal-card {
  transition: transform 0.26s var(--ease-out), opacity 0.22s var(--ease);
}

.sheet-enter-from,
.sheet-leave-to {
  opacity: 0;
}

.sheet-enter-from .modal-card,
.sheet-leave-to .modal-card {
  transform: translateY(14px) scale(0.985);
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .sheet-enter-active .modal-card,
  .sheet-leave-active .modal-card {
    transition: none;
  }
}

@media (max-width: 640px) {
  .banner {
    aspect-ratio: 3 / 1;
    min-height: 110px;
  }

  .card {
    margin: -36px 0.5rem 0;
    padding: 1rem 1.1rem 0.9rem;
  }

  .card-main {
    gap: 0.9rem;
  }

  .avatar-wrap {
    margin-top: -46px;
    transform: scale(0.8);
    transform-origin: left top;
  }

  /* 缩放后仍占原来的布局空间，否则资料卡会被顶上 */
  .avatar-wrap {
    margin-bottom: -19px;
  }

  .nickname {
    font-size: 1.2rem;
  }

  .stats {
    gap: 1.4rem;
  }
}

@media (prefers-reduced-motion: reduce) {
  .avatar-ring {
    animation: none;
  }
}
</style>
