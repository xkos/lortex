<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">{{ $t('keys.title') }}</h2>
      <el-button type="primary" @click="showCreate">
        <el-icon><Plus /></el-icon> {{ $t('keys.createKey') }}
      </el-button>
    </div>

    <el-table :data="keys" v-loading="loading" stripe>
      <el-table-column prop="name" :label="$t('common.name')" width="180" />
      <el-table-column prop="key_prefix" :label="$t('keys.key')" width="200">
        <template #default="{ row }">
          <span>{{ row.key_prefix }}</span>
          <el-button size="small" text @click="copyFullKey(row.id)">
            <el-icon><CopyDocument /></el-icon>
          </el-button>
        </template>
      </el-table-column>
      <el-table-column prop="default_model" :label="$t('keys.defaultModel')" width="240" />
      <el-table-column :label="$t('keys.modelsCol')" min-width="200">
        <template #default="{ row }">
          <el-tag v-for="m in row.model_group" :key="m" size="small" style="margin: 2px;">{{ m }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column :label="$t('keys.rpm')" width="80">
        <template #default="{ row }">
          <span v-if="row.rpm_limit > 0">{{ row.rpm_limit }}</span>
          <span v-else style="color: #909399;">-</span>
        </template>
      </el-table-column>
      <el-table-column :label="$t('keys.tpm')" width="100">
        <template #default="{ row }">
          <span v-if="row.tpm_limit > 0">{{ row.tpm_limit.toLocaleString() }}</span>
          <span v-else style="color: #909399;">-</span>
        </template>
      </el-table-column>
      <el-table-column prop="enabled" :label="$t('common.status')" width="100">
        <template #default="{ row }">
          <el-tag :type="row.enabled ? 'success' : 'danger'" size="small">
            {{ row.enabled ? $t('common.enabled') : $t('common.disabled') }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column :label="$t('common.actions')" width="320">
        <template #default="{ row }">
          <el-button size="small" @click="showEdit(row)">{{ $t('common.edit') }}</el-button>
          <el-button size="small" @click="showCopy(row)">{{ $t('keys.duplicate') }}</el-button>
          <el-popconfirm :title="$t('keys.confirmDelete')" @confirm="handleDelete(row.id)">
            <template #reference>
              <el-button size="small" type="danger">{{ $t('common.delete') }}</el-button>
            </template>
          </el-popconfirm>
        </template>
      </el-table-column>
    </el-table>

    <!-- Create Dialog -->
    <el-dialog v-model="createDialogVisible" :title="isDuplicating ? $t('keys.duplicateTitle') : $t('keys.createTitle')" width="550px">
      <el-form :model="createForm" label-width="130px">
        <el-form-item :label="$t('common.name')">
          <el-input v-model="createForm.name" :placeholder="$t('keys.namePlaceholder')" />
        </el-form-item>
        <el-form-item :label="$t('keys.modelGroup')">
          <el-select v-model="createForm.model_group" multiple :placeholder="$t('keys.selectModels')" style="width: 100%;">
            <el-option v-for="m in allModels" :key="modelId(m)" :label="modelId(m)" :value="modelId(m)" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('keys.defaultModel')">
          <el-select v-model="createForm.default_model" :placeholder="$t('keys.selectDefault')">
            <el-option v-for="m in createForm.model_group" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>

        <el-divider>{{ $t('keys.modelMap') }}</el-divider>
        <div v-for="(m, idx) in mappingList" :key="idx" style="display: flex; gap: 8px; margin-bottom: 8px; padding: 0 20px;">
          <el-input v-model="m.placeholder" :placeholder="$t('keys.placeholder')" style="flex: 1;" />
          <el-select v-model="m.model" :placeholder="$t('keys.targetModel')" style="flex: 1;" filterable>
            <el-option v-for="name in createForm.model_group" :key="name" :label="name" :value="name" />
          </el-select>
          <el-button type="danger" :icon="Delete" circle size="small" @click="mappingList.splice(idx, 1)" />
        </div>
        <el-form-item>
          <el-button @click="mappingList.push({ placeholder: '', model: '' })">
            <el-icon><Plus /></el-icon> {{ $t('keys.addMapping') }}
          </el-button>
          <el-button @click="quickAddClaude">{{ $t('keys.quickAddClaude') }}</el-button>
        </el-form-item>

        <el-form-item :label="$t('keys.rpmLimit')">
          <el-input-number v-model="createForm.rpm_limit" :min="0" :step="10" />
          <span style="margin-left: 8px; color: #909399;">{{ $t('keys.unlimitedHint') }}</span>
        </el-form-item>
        <el-form-item :label="$t('keys.tpmLimit')">
          <el-input-number v-model="createForm.tpm_limit" :min="0" :step="10000" />
          <span style="margin-left: 8px; color: #909399;">{{ $t('keys.unlimitedHint') }}</span>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createDialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleCreate" :loading="saving">{{ $t('common.create') }}</el-button>
      </template>
    </el-dialog>

    <!-- Created Key Dialog (shows full key once) -->
    <el-dialog v-model="keyCreatedVisible" :title="$t('keys.createdTitle')" width="500px" :close-on-click-modal="false">
      <el-alert type="warning" :closable="false" style="margin-bottom: 16px;">
        {{ $t('keys.copyWarning') }}
      </el-alert>
      <el-input :model-value="createdKey" readonly>
        <template #append>
          <el-button @click="copyKey">{{ $t('keys.copy') }}</el-button>
        </template>
      </el-input>
      <template #footer>
        <el-button type="primary" @click="keyCreatedVisible = false">{{ $t('keys.done') }}</el-button>
      </template>
    </el-dialog>

    <!-- Edit Dialog -->
    <el-dialog v-model="editDialogVisible" :title="$t('keys.editTitle')" width="550px">
      <el-form :model="editForm" label-width="130px">
        <el-form-item :label="$t('common.name')">
          <el-input v-model="editForm.name" />
        </el-form-item>
        <el-form-item :label="$t('keys.modelGroup')">
          <el-select v-model="editForm.model_group" multiple style="width: 100%;">
            <el-option v-for="m in allModels" :key="modelId(m)" :label="modelId(m)" :value="modelId(m)" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('keys.defaultModel')">
          <el-select v-model="editForm.default_model">
            <el-option v-for="m in editForm.model_group" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>

        <el-divider>{{ $t('keys.modelMap') }}</el-divider>
        <div v-for="(m, idx) in mappingList" :key="idx" style="display: flex; gap: 8px; margin-bottom: 8px; padding: 0 20px;">
          <el-input v-model="m.placeholder" :placeholder="$t('keys.placeholder')" style="flex: 1;" />
          <el-select v-model="m.model" :placeholder="$t('keys.targetModel')" style="flex: 1;" filterable>
            <el-option v-for="name in editForm.model_group" :key="name" :label="name" :value="name" />
          </el-select>
          <el-button type="danger" :icon="Delete" circle size="small" @click="mappingList.splice(idx, 1)" />
        </div>
        <el-form-item>
          <el-button @click="mappingList.push({ placeholder: '', model: '' })">
            <el-icon><Plus /></el-icon> {{ $t('keys.addMapping') }}
          </el-button>
          <el-button @click="quickAddClaude">{{ $t('keys.quickAddClaude') }}</el-button>
        </el-form-item>

        <el-form-item :label="$t('keys.rpmLimit')">
          <el-input-number v-model="editForm.rpm_limit" :min="0" :step="10" />
          <span style="margin-left: 8px; color: #909399;">{{ $t('keys.unlimitedHint') }}</span>
        </el-form-item>
        <el-form-item :label="$t('keys.tpmLimit')">
          <el-input-number v-model="editForm.tpm_limit" :min="0" :step="10000" />
          <span style="margin-left: 8px; color: #909399;">{{ $t('keys.unlimitedHint') }}</span>
        </el-form-item>
        <el-form-item :label="$t('common.enabled')">
          <el-switch v-model="editForm.enabled" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editDialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleUpdate" :loading="saving">{{ $t('common.save') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Delete } from '@element-plus/icons-vue'
import { useI18n } from 'vue-i18n'
import api from '../api'

const { t } = useI18n()

const keys = ref<any[]>([])
const allModels = ref<any[]>([])
const loading = ref(false)
const saving = ref(false)

const createDialogVisible = ref(false)
const keyCreatedVisible = ref(false)
const editDialogVisible = ref(false)
const isDuplicating = ref(false)
const createdKey = ref('')
const mappingList = ref<{ placeholder: string; model: string }[]>([])

const CLAUDE_PRESETS = [
  'ANTHROPIC_MODEL',
  'ANTHROPIC_DEFAULT_SONNET_MODEL',
  'ANTHROPIC_DEFAULT_OPUS_MODEL',
  'ANTHROPIC_DEFAULT_HAIKU_MODEL',
  'ANTHROPIC_REASONING_MODEL',
]

function quickAddClaude() {
  const existing = new Set(mappingList.value.map(m => m.placeholder))
  for (const name of CLAUDE_PRESETS) {
    if (!existing.has(name)) {
      mappingList.value.push({ placeholder: name, model: '' })
    }
  }
}

const createForm = ref({
  name: '',
  model_group: [] as string[],
  default_model: '',
  fallback_models: [] as string[],
  rpm_limit: 0,
  tpm_limit: 0,
})

const editForm = ref({
  id: '',
  name: '',
  model_group: [] as string[],
  default_model: '',
  rpm_limit: 0,
  tpm_limit: 0,
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
    ElMessage.error(t('keys.loadFailed'))
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
  createForm.value = { name: '', model_group: [], default_model: '', fallback_models: [], rpm_limit: 0, tpm_limit: 0 }
  mappingList.value = []
  isDuplicating.value = false
  createDialogVisible.value = true
}

function showCopy(row: any) {
  createForm.value = {
    name: `${row.name} ${t('keys.copySuffix')}`,
    model_group: [...(row.model_group || [])],
    default_model: row.default_model || '',
    fallback_models: [...(row.fallback_models || [])],
    rpm_limit: row.rpm_limit || 0,
    tpm_limit: row.tpm_limit || 0,
  }
  const mm = row.model_map || {}
  mappingList.value = Object.entries(mm).map(([placeholder, model]) => ({ placeholder, model: model as string }))
  isDuplicating.value = true
  createDialogVisible.value = true
}

function showEdit(row: any) {
  editForm.value = {
    id: row.id,
    name: row.name,
    model_group: row.model_group || [],
    default_model: row.default_model,
    rpm_limit: row.rpm_limit || 0,
    tpm_limit: row.tpm_limit || 0,
    enabled: row.enabled,
  }
  const mm = row.model_map || {}
  mappingList.value = Object.entries(mm).map(([placeholder, model]) => ({ placeholder, model: model as string }))
  editDialogVisible.value = true
}

function buildModelMap(): Record<string, string> | null {
  const map: Record<string, string> = {}
  for (const m of mappingList.value) {
    if (m.placeholder.trim() && m.model) map[m.placeholder.trim()] = m.model
  }
  return Object.keys(map).length > 0 ? map : null
}

async function handleCreate() {
  saving.value = true
  try {
    const payload = { ...createForm.value, model_map: buildModelMap() }
    const { data } = await api.post('/keys', payload)
    createdKey.value = data.key
    createDialogVisible.value = false
    keyCreatedVisible.value = true
    await fetchKeys()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || t('keys.createFailed'))
  } finally {
    saving.value = false
  }
}

async function handleUpdate() {
  saving.value = true
  try {
    const payload = { ...editForm.value, model_map: buildModelMap() }
    await api.put(`/keys/${editForm.value.id}`, payload)
    ElMessage.success(t('keys.keyUpdated'))
    editDialogVisible.value = false
    await fetchKeys()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || t('keys.updateFailed'))
  } finally {
    saving.value = false
  }
}

async function handleDelete(id: string) {
  try {
    await api.delete(`/keys/${id}`)
    ElMessage.success(t('keys.keyDeleted'))
    await fetchKeys()
  } catch {
    ElMessage.error(t('common.deleteFailed'))
  }
}

function copyKey() {
  navigator.clipboard.writeText(createdKey.value)
  ElMessage.success(t('keys.copied'))
}

async function copyFullKey(id: string) {
  try {
    const { data } = await api.get(`/keys/${id}/reveal`)
    await navigator.clipboard.writeText(data.key)
    ElMessage.success(t('keys.keyCopied'))
  } catch {
    ElMessage.error(t('keys.copyFailed'))
  }
}

onMounted(() => {
  fetchKeys()
  fetchModels()
})
</script>
