<template>
  <div class="login-container">
    <el-card class="login-card">
      <template #header>
        <h2>Lortex Proxy Admin</h2>
      </template>
      <el-form @submit.prevent="handleLogin">
        <el-form-item label="Admin Key">
          <el-input
            v-model="adminKey"
            type="password"
            placeholder="Enter admin key"
            show-password
            @keyup.enter="handleLogin"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleLogin" :loading="loading" style="width: 100%">
            Login
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import api from '../api'

const router = useRouter()
const adminKey = ref('')
const loading = ref(false)

async function handleLogin() {
  if (!adminKey.value.trim()) {
    ElMessage.warning('Please enter admin key')
    return
  }

  loading.value = true
  // 验证 key：尝试调用 providers 列表
  sessionStorage.setItem('admin_key', adminKey.value.trim())
  try {
    await api.get('/providers')
    ElMessage.success('Login successful')
    router.push('/')
  } catch (e: any) {
    sessionStorage.removeItem('admin_key')
    ElMessage.error('Invalid admin key')
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.login-container {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 100vh;
  background: #f5f7fa;
}
.login-card {
  width: 400px;
}
.login-card h2 {
  margin: 0;
  text-align: center;
}
</style>
