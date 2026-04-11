<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">Providers</h2>
      <el-button type="primary" @click="showCreate">
        <el-icon><Plus /></el-icon> Add Provider
      </el-button>
    </div>

    <el-table :data="providers" v-loading="loading" stripe>
      <el-table-column prop="id" label="ID" width="180" />
      <el-table-column prop="vendor" label="Vendor" width="120" />
      <el-table-column prop="display_name" label="Name" />
      <el-table-column prop="base_url" label="Base URL" />
      <el-table-column prop="enabled" label="Status" width="100">
        <template #default="{ row }">
          <el-tag :type="row.enabled ? 'success' : 'danger'" size="small">
            {{ row.enabled ? 'Enabled' : 'Disabled' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="Actions" width="180">
        <template #default="{ row }">
          <el-button size="small" @click="showEdit(row)">Edit</el-button>
          <el-popconfirm title="Delete this provider?" @confirm="handleDelete(row.id)">
            <template #reference>
              <el-button size="small" type="danger">Delete</el-button>
            </template>
          </el-popconfirm>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialogVisible" :title="isEdit ? 'Edit Provider' : 'Add Provider'" width="500px">
      <el-form :model="form" label-width="100px">
        <el-form-item label="ID" v-if="!isEdit">
          <el-input v-model="form.id" placeholder="e.g. openai-main" />
        </el-form-item>
        <el-form-item label="Vendor">
          <el-select v-model="form.vendor" placeholder="Select vendor">
            <el-option label="OpenAI" value="openai" />
            <el-option label="Anthropic" value="anthropic" />
            <el-option label="DeepSeek" value="deepseek" />
            <el-option label="Custom" value="custom" />
          </el-select>
        </el-form-item>
        <el-form-item label="Name">
          <el-input v-model="form.display_name" />
        </el-form-item>
        <el-form-item label="API Key">
          <el-input v-model="form.api_key" type="password" show-password />
        </el-form-item>
        <el-form-item label="Base URL">
          <el-input v-model="form.base_url" placeholder="https://api.openai.com/v1" />
        </el-form-item>
        <el-form-item label="Enabled">
          <el-switch v-model="form.enabled" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">Cancel</el-button>
        <el-button type="primary" @click="handleSave" :loading="saving">Save</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import api from '../api'

interface Provider {
  id: string
  vendor: string
  display_name: string
  api_key: string
  base_url: string
  enabled: boolean
}

const providers = ref<Provider[]>([])
const loading = ref(false)
const dialogVisible = ref(false)
const isEdit = ref(false)
const saving = ref(false)

const emptyForm = (): Provider => ({
  id: '', vendor: 'openai', display_name: '', api_key: '', base_url: 'https://api.openai.com/v1', enabled: true,
})
const form = ref<Provider>(emptyForm())

async function fetchProviders() {
  loading.value = true
  try {
    const { data } = await api.get('/providers')
    providers.value = data
  } catch (e: any) {
    ElMessage.error('Failed to load providers')
  } finally {
    loading.value = false
  }
}

function showCreate() {
  isEdit.value = false
  form.value = emptyForm()
  dialogVisible.value = true
}

function showEdit(row: Provider) {
  isEdit.value = true
  form.value = { ...row }
  dialogVisible.value = true
}

async function handleSave() {
  saving.value = true
  try {
    if (isEdit.value) {
      await api.put(`/providers/${form.value.id}`, form.value)
      ElMessage.success('Provider updated')
    } else {
      await api.post('/providers', form.value)
      ElMessage.success('Provider created')
    }
    dialogVisible.value = false
    await fetchProviders()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || 'Save failed')
  } finally {
    saving.value = false
  }
}

async function handleDelete(id: string) {
  try {
    await api.delete(`/providers/${id}`)
    ElMessage.success('Provider deleted')
    await fetchProviders()
  } catch (e: any) {
    ElMessage.error('Delete failed')
  }
}

onMounted(fetchProviders)
</script>
