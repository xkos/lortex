<template>
  <div class="login-container">
    <el-card class="login-card">
      <template #header>
        <h2>{{ $t('login.title') }}</h2>
      </template>
      <el-form @submit.prevent="handleLogin">
        <el-form-item :label="$t('login.adminKey')">
          <el-input
            v-model="adminKey"
            type="password"
            :placeholder="$t('login.placeholder')"
            show-password
            @keyup.enter="handleLogin"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleLogin" :loading="loading" style="width: 100%">
            {{ $t('login.login') }}
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
import { useI18n } from 'vue-i18n'
import api from '../api'

const { t } = useI18n()
const router = useRouter()
const adminKey = ref('')
const loading = ref(false)

async function handleLogin() {
  if (!adminKey.value.trim()) {
    ElMessage.warning(t('login.pleaseEnter'))
    return
  }

  loading.value = true
  sessionStorage.setItem('admin_key', adminKey.value.trim())
  try {
    await api.get('/providers')
    ElMessage.success(t('login.success'))
    router.push('/')
  } catch (e: any) {
    sessionStorage.removeItem('admin_key')
    ElMessage.error(t('login.invalid'))
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
