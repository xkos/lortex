<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">API Keys</h2>
      <el-button type="primary" @click="showCreate">
        <el-icon><Plus /></el-icon> Create Key
      </el-button>
    </div>

    <el-table :data="keys" v-loading="loading" stripe>
      <el-table-column prop="name" label="Name" width="180" />
      <el-table-column prop="key_prefix" label="Key" width="200">
        <template #default="{ row }">
          <span>{{ row.key_prefix }}</span>
          <el-button size="small" text @click="copyFullKey(row.id)">
            <el-icon><CopyDocument /></el-icon>
          </el-button>
        </template>
      </el-table-column>
      <el-table-column prop="default_model" label="Default Model" width="240" />
      <el-table-column label="Models" min-width="200">
        <template #default="{ row }">
          <el-tag v-for="m in row.model_group" :key="m" size="small" style="margin: 2px;">{{ m }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="Credits" width="180">
        <template #default="{ row }">
          <span>{{ row.credit_used.toLocaleString() }}</span>
          <span v-if="row.credit_limit > 0"> / {{ row.credit_limit.toLocaleString() }}</span>
          <span v-else> / unlimited</span>
        </template>
      </el-table-column>
      <el-table-column prop="enabled" label="Status" width="100">
        <template #default="{ row }">
          <el-tag :type="row.enabled ? 'success' : 'danger'" size="small">
            {{ row.enabled ? 'Enabled' : 'Disabled' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="Actions" width="260">
        <template #default="{ row }">
          <el-button size="small" @click="showEdit(row)">Edit</el-button>
          <el-popconfirm title="Reset credits to 0?" @confirm="handleResetCredits(row.id)">
            <template #reference>
              <el-button size="small" type="warning">Reset</el-button>
            </template>
          </el-popconfirm>
          <el-popconfirm title="Delete this key?" @confirm="handleDelete(row.id)">
            <template #reference>
              <el-button size="small" type="danger">Delete</el-button>
            </template>
          </el-popconfirm>
        </template>
      </el-table-column>
    </el-table>

    <!-- Create Dialog -->
    <el-dialog v-model="createDialogVisible" title="Create API Key" width="550px">
      <el-form :model="createForm" label-width="130px">
        <el-form-item label="Name">
          <el-input v-model="createForm.name" placeholder="e.g. cursor-personal" />
        </el-form-item>
        <el-form-item label="Model Group">
          <el-select v-model="createForm.model_group" multiple placeholder="Select models" style="width: 100%;">
            <el-option v-for="m in allModels" :key="modelId(m)" :label="modelId(m)" :value="modelId(m)" />
          </el-select>
        </el-form-item>
        <el-form-item label="Default Model">
          <el-select v-model="createForm.default_model" placeholder="Select default">
            <el-option v-for="m in createForm.model_group" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="Credit Limit">
          <el-input-number v-model="createForm.credit_limit" :min="0" :step="10000" />
          <span style="margin-left: 8px; color: #909399;">0 = unlimited</span>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createDialogVisible = false">Cancel</el-button>
        <el-button type="primary" @click="handleCreate" :loading="saving">Create</el-button>
      </template>
    </el-dialog>

    <!-- Created Key Dialog (shows full key once) -->
    <el-dialog v-model="keyCreatedVisible" title="API Key Created" width="500px" :close-on-click-modal="false">
      <el-alert type="warning" :closable="false" style="margin-bottom: 16px;">
        Copy this key now. It will not be shown again.
      </el-alert>
      <el-input :model-value="createdKey" readonly>
        <template #append>
          <el-button @click="copyKey">Copy</el-button>
        </template>
      </el-input>
      <template #footer>
        <el-button type="primary" @click="keyCreatedVisible = false">Done</el-button>
      </template>
    </el-dialog>

    <!-- Edit Dialog -->
    <el-dialog v-model="editDialogVisible" title="Edit API Key" width="550px">
      <el-form :model="editForm" label-width="130px">
        <el-form-item label="Name">
          <el-input v-model="editForm.name" />
        </el-form-item>
        <el-form-item label="Model Group">
          <el-select v-model="editForm.model_group" multiple style="width: 100%;">
            <el-option v-for="m in allModels" :key="modelId(m)" :label="modelId(m)" :value="modelId(m)" />
          </el-select>
        </el-form-item>
        <el-form-item label="Default Model">
          <el-select v-model="editForm.default_model">
            <el-option v-for="m in editForm.model_group" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="Credit Limit">
          <el-input-number v-model="editForm.credit_limit" :min="0" :step="10000" />
        </el-form-item>
        <el-form-item label="Enabled">
          <el-switch v-model="editForm.enabled" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editDialogVisible = false">Cancel</el-button>
        <el-button type="primary" @click="handleUpdate" :loading="saving">Save</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import api from '../api'

const keys = ref<any[]>([])
const allModels = ref<any[]>([])
const loading = ref(false)
const saving = ref(false)

const createDialogVisible = ref(false)
const keyCreatedVisible = ref(false)
const editDialogVisible = ref(false)
const createdKey = ref('')

const createForm = ref({
  name: '',
  model_group: [] as string[],
  default_model: '',
  credit_limit: 0,
})

const editForm = ref({
  id: '',
  name: '',
  model_group: [] as string[],
  default_model: '',
  credit_limit: 0,
  enabled: true,
})

function modelId(m: any) {
  return `${m.provider_id}/${m.vendor_model_name}`
}

async function fetchKeys() {
  loading.value = true
  try {
    const { data } = await api.get('/keys')
    keys.value = data
  } catch {
    ElMessage.error('Failed to load API keys')
  } finally {
    loading.value = false
  }
}

async function fetchModels() {
  try {
    const { data } = await api.get('/models')
    allModels.value = data
  } catch {}
}

function showCreate() {
  createForm.value = { name: '', model_group: [], default_model: '', credit_limit: 0 }
  createDialogVisible.value = true
}

function showEdit(row: any) {
  editForm.value = {
    id: row.id,
    name: row.name,
    model_group: row.model_group || [],
    default_model: row.default_model,
    credit_limit: row.credit_limit,
    enabled: row.enabled,
  }
  editDialogVisible.value = true
}

async function handleCreate() {
  saving.value = true
  try {
    const { data } = await api.post('/keys', createForm.value)
    createdKey.value = data.key
    createDialogVisible.value = false
    keyCreatedVisible.value = true
    await fetchKeys()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || 'Create failed')
  } finally {
    saving.value = false
  }
}

async function handleUpdate() {
  saving.value = true
  try {
    await api.put(`/keys/${editForm.value.id}`, editForm.value)
    ElMessage.success('Key updated')
    editDialogVisible.value = false
    await fetchKeys()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || 'Update failed')
  } finally {
    saving.value = false
  }
}

async function handleResetCredits(id: string) {
  try {
    await api.post(`/keys/${id}/reset-credits`)
    ElMessage.success('Credits reset')
    await fetchKeys()
  } catch {
    ElMessage.error('Reset failed')
  }
}

async function handleDelete(id: string) {
  try {
    await api.delete(`/keys/${id}`)
    ElMessage.success('Key deleted')
    await fetchKeys()
  } catch {
    ElMessage.error('Delete failed')
  }
}

function copyKey() {
  navigator.clipboard.writeText(createdKey.value)
  ElMessage.success('Copied to clipboard')
}

async function copyFullKey(id: string) {
  try {
    const { data } = await api.get(`/keys/${id}/reveal`)
    await navigator.clipboard.writeText(data.key)
    ElMessage.success('Key copied to clipboard')
  } catch {
    ElMessage.error('Failed to copy key')
  }
}

onMounted(() => {
  fetchKeys()
  fetchModels()
})
</script>
