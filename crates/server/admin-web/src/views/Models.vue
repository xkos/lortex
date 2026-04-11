<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">Models</h2>
      <el-button type="primary" @click="showCreate">
        <el-icon><Plus /></el-icon> Add Model
      </el-button>
    </div>

    <el-table :data="models" v-loading="loading" stripe>
      <el-table-column label="ID" width="280">
        <template #default="{ row }">
          {{ row.provider_id }}/{{ row.vendor_model_name }}
        </template>
      </el-table-column>
      <el-table-column prop="display_name" label="Name" width="180" />
      <el-table-column prop="model_type" label="Type" width="100" />
      <el-table-column label="Multiplier" width="160">
        <template #default="{ row }">
          In: {{ row.input_multiplier }}x / Out: {{ row.output_multiplier }}x
        </template>
      </el-table-column>
      <el-table-column label="Capabilities" min-width="200">
        <template #default="{ row }">
          <el-tag v-if="row.supports_tools" size="small" style="margin: 2px;">Tools</el-tag>
          <el-tag v-if="row.supports_vision" size="small" style="margin: 2px;">Vision</el-tag>
          <el-tag v-if="row.supports_streaming" size="small" style="margin: 2px;">Stream</el-tag>
          <el-tag v-if="row.supports_cache" size="small" style="margin: 2px;">Cache</el-tag>
          <el-tag v-if="row.supports_structured_output" size="small" style="margin: 2px;">JSON</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="enabled" label="Status" width="100">
        <template #default="{ row }">
          <el-tag :type="row.enabled ? 'success' : 'danger'" size="small">
            {{ row.enabled ? 'Enabled' : 'Disabled' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="Actions" width="140">
        <template #default="{ row }">
          <el-popconfirm title="Delete this model?" @confirm="handleDelete(row.provider_id, row.vendor_model_name)">
            <template #reference>
              <el-button size="small" type="danger">Delete</el-button>
            </template>
          </el-popconfirm>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialogVisible" title="Add Model" width="600px">
      <el-form :model="form" label-width="140px">
        <el-form-item label="Provider ID">
          <el-select v-model="form.provider_id" placeholder="Select provider">
            <el-option v-for="p in providers" :key="p.id" :label="p.display_name" :value="p.id" />
          </el-select>
        </el-form-item>
        <el-form-item label="Model Name">
          <el-input v-model="form.vendor_model_name" placeholder="e.g. gpt-4o" />
        </el-form-item>
        <el-form-item label="Display Name">
          <el-input v-model="form.display_name" />
        </el-form-item>
        <el-form-item label="Aliases">
          <el-input v-model="aliasesStr" placeholder="Comma separated, e.g. gpt4,gpt-4" />
        </el-form-item>
        <el-form-item label="Type">
          <el-select v-model="form.model_type">
            <el-option label="Chat" value="chat" />
            <el-option label="Embedding" value="embedding" />
            <el-option label="Image Generation" value="image_generation" />
            <el-option label="TTS" value="tts" />
            <el-option label="STT" value="stt" />
          </el-select>
        </el-form-item>

        <el-divider>Capabilities</el-divider>
        <el-form-item label="Features">
          <el-checkbox v-model="form.supports_streaming">Streaming</el-checkbox>
          <el-checkbox v-model="form.supports_tools">Tools</el-checkbox>
          <el-checkbox v-model="form.supports_vision">Vision</el-checkbox>
          <el-checkbox v-model="form.supports_cache">Cache</el-checkbox>
          <el-checkbox v-model="form.supports_structured_output">Structured Output</el-checkbox>
          <el-checkbox v-model="form.supports_prefill">Prefill</el-checkbox>
          <el-checkbox v-model="form.supports_web_search">Web Search</el-checkbox>
        </el-form-item>
        <el-form-item label="Context Window">
          <el-input-number v-model="form.context_window" :min="0" :step="1000" />
        </el-form-item>

        <el-divider>Pricing (credits per 1k tokens)</el-divider>
        <el-form-item label="Input">
          <el-input-number v-model="form.input_multiplier" :min="0" :precision="2" :step="0.1" />
        </el-form-item>
        <el-form-item label="Output">
          <el-input-number v-model="form.output_multiplier" :min="0" :precision="2" :step="0.1" />
        </el-form-item>
        <el-form-item label="Cache Write">
          <el-input-number v-model="form.cache_write_multiplier" :min="0" :precision="2" :step="0.1" />
        </el-form-item>
        <el-form-item label="Cache Read">
          <el-input-number v-model="form.cache_read_multiplier" :min="0" :precision="2" :step="0.1" />
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

const models = ref<any[]>([])
const providers = ref<any[]>([])
const loading = ref(false)
const dialogVisible = ref(false)
const saving = ref(false)
const aliasesStr = ref('')

const emptyForm = () => ({
  provider_id: '',
  vendor_model_name: '',
  display_name: '',
  aliases: [] as string[],
  model_type: 'chat',
  supports_streaming: true,
  supports_tools: false,
  supports_structured_output: false,
  supports_vision: false,
  supports_prefill: false,
  supports_cache: false,
  supports_web_search: false,
  supports_batch: false,
  context_window: 128000,
  cache_enabled: true,
  input_multiplier: 1.0,
  output_multiplier: 1.0,
  cache_write_multiplier: 0,
  cache_read_multiplier: 0,
  enabled: true,
})
const form = ref(emptyForm())

async function fetchModels() {
  loading.value = true
  try {
    const { data } = await api.get('/models')
    models.value = data
  } catch {
    ElMessage.error('Failed to load models')
  } finally {
    loading.value = false
  }
}

async function fetchProviders() {
  try {
    const { data } = await api.get('/providers')
    providers.value = data
  } catch {}
}

function showCreate() {
  form.value = emptyForm()
  aliasesStr.value = ''
  dialogVisible.value = true
}

async function handleSave() {
  saving.value = true
  const payload = {
    ...form.value,
    aliases: aliasesStr.value ? aliasesStr.value.split(',').map(s => s.trim()).filter(Boolean) : [],
    cache_write_multiplier: form.value.cache_write_multiplier || null,
    cache_read_multiplier: form.value.cache_read_multiplier || null,
  }
  try {
    await api.post('/models', payload)
    ElMessage.success('Model created')
    dialogVisible.value = false
    await fetchModels()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || 'Save failed')
  } finally {
    saving.value = false
  }
}

async function handleDelete(providerId: string, modelName: string) {
  try {
    await api.delete(`/models/${providerId}/${modelName}`)
    ElMessage.success('Model deleted')
    await fetchModels()
  } catch {
    ElMessage.error('Delete failed')
  }
}

onMounted(() => {
  fetchModels()
  fetchProviders()
})
</script>
