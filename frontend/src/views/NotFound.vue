<template>
  <div class="not-found">
    <h1 class="code">404</h1>
    <p class="message">页面走丢了，这里什么都没有 🙈</p>
    <div class="actions">
      <el-button type="primary" @click="$router.push('/')">返回首页</el-button>
      <el-button @click="$router.push('/blog')">去看看博客</el-button>
    </div>
  </div>
</template>

<script setup>
</script>

<style scoped>
.not-found {
  text-align: center;
  padding: 80px 20px;
}

.code {
  font-size: 5rem;
  font-weight: 800;
  background: linear-gradient(135deg, var(--accent), #ffa502);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  margin-bottom: 10px;
}

.message {
  color: var(--text-secondary);
  margin-bottom: 24px;
}

.actions {
  display: flex;
  gap: 12px;
  justify-content: center;
  flex-wrap: wrap;
}
</style>
