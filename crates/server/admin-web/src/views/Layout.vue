<template>
  <el-container style="min-height: 100vh">
    <el-aside width="200px">
      <el-menu
        :default-active="activeMenu"
        router
        style="height: 100%"
      >
        <div style="padding: 16px; text-align: center; font-weight: bold; font-size: 16px;">
          {{ $t('layout.brand') }}
        </div>
        <el-menu-item index="/providers">
          <el-icon><Connection /></el-icon>
          <span>{{ $t('layout.providersModels') }}</span>
        </el-menu-item>
        <el-menu-item index="/keys">
          <el-icon><Key /></el-icon>
          <span>{{ $t('layout.apiKeys') }}</span>
        </el-menu-item>
        <el-menu-item index="/usage">
          <el-icon><DataAnalysis /></el-icon>
          <span>{{ $t('layout.usage') }}</span>
        </el-menu-item>
      </el-menu>
    </el-aside>
    <el-container>
      <el-header style="display: flex; align-items: center; justify-content: flex-end; border-bottom: 1px solid #e4e7ed;">
        <el-button text @click="toggleLocale" style="margin-right: 8px;">
          {{ locale === 'zh' ? 'EN' : '中文' }}
        </el-button>
        <el-button text @click="handleLogout">
          <el-icon><SwitchButton /></el-icon>
          {{ $t('layout.logout') }}
        </el-button>
      </el-header>
      <el-main>
        <router-view />
      </el-main>
    </el-container>
  </el-container>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

const { locale } = useI18n()
const route = useRoute()
const router = useRouter()

const activeMenu = computed(() => route.path)

function toggleLocale() {
  const next = locale.value === 'zh' ? 'en' : 'zh'
  locale.value = next
  localStorage.setItem('locale', next)
}

function handleLogout() {
  sessionStorage.removeItem('admin_key')
  router.push('/login')
}
</script>
