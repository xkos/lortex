<template>
  <el-container style="min-height: 100vh">
    <el-aside width="200px">
      <el-menu
        :default-active="activeMenu"
        router
        style="height: 100%"
      >
        <div style="padding: 16px; text-align: center; font-weight: bold; font-size: 16px;">
          Lortex Proxy
        </div>
        <el-menu-item index="/providers">
          <el-icon><Connection /></el-icon>
          <span>Providers</span>
        </el-menu-item>
        <el-menu-item index="/models">
          <el-icon><Cpu /></el-icon>
          <span>Models</span>
        </el-menu-item>
        <el-menu-item index="/keys">
          <el-icon><Key /></el-icon>
          <span>API Keys</span>
        </el-menu-item>
        <el-menu-item index="/usage">
          <el-icon><DataAnalysis /></el-icon>
          <span>Usage</span>
        </el-menu-item>
      </el-menu>
    </el-aside>
    <el-container>
      <el-header style="display: flex; align-items: center; justify-content: flex-end; border-bottom: 1px solid #e4e7ed;">
        <el-button text @click="handleLogout">
          <el-icon><SwitchButton /></el-icon>
          Logout
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

const route = useRoute()
const router = useRouter()

const activeMenu = computed(() => route.path)

function handleLogout() {
  sessionStorage.removeItem('admin_key')
  router.push('/login')
}
</script>
