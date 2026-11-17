<template>
  <div class="not-found">
    <h1 class="code">404</h1>
    <p class="message">页面走丢了，这里什么都没有 🙈</p>
    <div class="actions">
      <router-link to="/" class="btn">返回首页</router-link>
      <router-link to="/blog" class="btn btn-outline">去看看博客</router-link>
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
