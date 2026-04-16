<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">{{ $t('models.title') }}</h2>
      <el-button type="primary" @click="showCreate">
        <el-icon><Plus /></el-icon> {{ $t('models.add') }}
      </el-button>
    </div>

    <el-table :data="models" v-loading="loading" stripe>
      <el-table-column :label="$t('common.id')" width="280">
        <template #default="{ row }">
          {{ row.provider_id }}/{{ row.vendor_model_name }}
        </template>
      </el-table-column>
      <el-table-column prop="display_name" :label="$t('common.name')" width="180" />
      <el-table-column prop="model_type" :label="$t('models.type')" width="100" />
      <el-table-column :label="$t('models.apiFormats')" width="160">
        <template #default="{ row }">
          <el-tag v-for="f in row.api_formats" :key="f" size="small" style="margin: 2px;">{{ f }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column :label="$t('models.multiplier')" width="160">
        <template #default="{ row }">
          {{ $t('models.inOut', { input: row.input_multiplier, output: row.output_multiplier }) }}
        </template>
      </el-table-column>
      <el-table-column :label="$t('models.capabilities')" min-width="200">
        <template #default="{ row }">
          <el-tag v-if="row.supports_tools" size="small" style="margin: 2px;">{{ $t('models.tools') }}</el-tag>
          <el-tag v-if="row.supports_vision" size="small" style="margin: 2px;">{{ $t('models.vision') }}</el-tag>
          <el-tag v-if="row.supports_streaming" size="small" style="margin: 2px;">{{ $t('models.stream') }}</el-tag>
          <el-tag v-if="row.supports_cache" size="small" style="margin: 2px;">{{ $t('models.cache') }}</el-tag>
          <el-tag v-if="row.supports_structured_output" size="small" style="margin: 2px;">{{ $t('models.json') }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="enabled" :label="$t('common.status')" width="100">
        <template #default="{ row }">
          <el-tag :type="row.enabled ? 'success' : 'danger'" size="small">
            {{ row.enabled ? $t('common.enabled') : $t('common.disabled') }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column :label="$t('common.actions')" width="180">
        <template #default="{ row }">
          <el-button size="small" @click="showEdit(row)">{{ $t('common.edit') }}</el-button>
          <el-popconfirm :title="$t('models.confirmDelete')" @confirm="handleDelete(row.provider_id, row.vendor_model_name)">
            <template #reference>
              <el-button size="small" type="danger">{{ $t('common.delete') }}</el-button>
            </template>
          </el-popconfirm>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialogVisible" :title="isEdit ? $t('models.editTitle') : $t('models.addTitle')" width="600px">
      <el-form :model="form" label-width="140px">
        <el-form-item :label="$t('models.providerId')" v-if="!isEdit">
          <el-select v-model="form.provider_id" :placeholder="$t('models.selectProvider')">
            <el-option v-for="p in providers" :key="p.id" :label="p.display_name" :value="p.id" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('models.modelName')" v-if="!isEdit">
          <el-input v-model="form.vendor_model_name" :placeholder="$t('models.modelNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="$t('models.providerModel')" v-if="isEdit">
          <el-input :model-value="`${form.provider_id}/${form.vendor_model_name}`" disabled />
        </el-form-item>
        <el-form-item :label="$t('models.displayName')">
          <el-input v-model="form.display_name" />
        </el-form-item>
        <el-form-item :label="$t('models.aliases')">
          <el-input v-model="aliasesStr" :placeholder="$t('models.aliasesPlaceholder')" />
        </el-form-item>
        <el-form-item :label="$t('models.type')">
          <el-select v-model="form.model_type">
            <el-option :label="$t('models.chat')" value="chat" />
            <el-option :label="$t('models.embedding')" value="embedding" />
            <el-option :label="$t('models.imageGeneration')" value="image_generation" />
            <el-option :label="$t('models.tts')" value="tts" />
            <el-option :label="$t('models.stt')" value="stt" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('models.apiFormats')">
          <el-checkbox-group v-model="form.api_formats">
            <el-checkbox label="openai" value="openai">OpenAI</el-checkbox>
            <el-checkbox label="anthropic" value="anthropic">Anthropic</el-checkbox>
          </el-checkbox-group>
        </el-form-item>

        <el-divider>{{ $t('models.capabilities') }}</el-divider>
        <el-form-item :label="$t('models.features')">
          <el-checkbox v-model="form.supports_streaming">{{ $t('models.streaming') }}</el-checkbox>
          <el-checkbox v-model="form.supports_tools">{{ $t('models.tools') }}</el-checkbox>
          <el-checkbox v-model="form.supports_vision">{{ $t('models.vision') }}</el-checkbox>
          <el-checkbox v-model="form.supports_cache">{{ $t('models.cache') }}</el-checkbox>
          <el-checkbox v-model="form.supports_structured_output">{{ $t('models.structuredOutput') }}</el-checkbox>
          <el-checkbox v-model="form.supports_prefill">{{ $t('models.prefill') }}</el-checkbox>
          <el-checkbox v-model="form.supports_web_search">{{ $t('models.webSearch') }}</el-checkbox>
        </el-form-item>
        <el-form-item :label="$t('models.contextWindow')">
          <el-input-number v-model="form.context_window" :min="0" :step="1000" />
        </el-form-item>

        <el-divider>{{ $t('models.pricing') }}</el-divider>
        <el-form-item :label="$t('models.input')">
          <el-input-number v-model="form.input_multiplier" :min="0" :precision="2" :step="0.1" />
        </el-form-item>
        <el-form-item :label="$t('models.output')">
          <el-input-number v-model="form.output_multiplier" :min="0" :precision="2" :step="0.1" />
        </el-form-item>
        <el-form-item :label="$t('models.cacheWrite')">
          <el-input-number v-model="form.cache_write_multiplier" :min="0" :precision="2" :step="0.1" />
        </el-form-item>
        <el-form-item :label="$t('models.cacheRead')">
          <el-input-number v-model="form.cache_read_multiplier" :min="0" :precision="2" :step="0.1" />
        </el-form-item>

        <el-divider>{{ $t('models.rateLimits') }}</el-divider>
        <el-form-item :label="$t('models.rpmLimit')">
          <el-input-number v-model="form.rpm_limit" :min="0" :step="10" />
          <span style="margin-left: 8px; color: #909399;">{{ $t('models.unlimitedHint') }}</span>
        </el-form-item>
        <el-form-item :label="$t('models.tpmLimit')">
          <el-input-number v-model="form.tpm_limit" :min="0" :step="10000" />
          <span style="margin-left: 8px; color: #909399;">{{ $t('models.unlimitedHint') }}</span>
        </el-form-item>

        <el-divider>{{ $t('models.extraHeaders') }}</el-divider>
        <div v-for="(h, idx) in headerList" :key="idx" style="display: flex; gap: 8px; margin-bottom: 8px; padding: 0 20px;">
          <el-input v-model="h.key" :placeholder="$t('models.headerKey')" style="flex: 1;" />
          <el-input v-model="h.value" :placeholder="$t('models.headerValue')" style="flex: 1;" />
          <el-button type="danger" :icon="Delete" circle size="small" @click="headerList.splice(idx, 1)" />
        </div>
        <el-form-item>
          <el-button @click="headerList.push({ key: '', value: '' })">
            <el-icon><Plus /></el-icon> {{ $t('models.addHeader') }}
          </el-button>
        </el-form-item>

        <el-divider>{{ $t('common.status') }}</el-divider>
        <el-form-item :label="$t('common.enabled')">
          <el-switch v-model="form.enabled" />
        </el-form-item>
        <el-form-item :label="$t('models.cacheEnabled')">
          <el-switch v-model="form.cache_enabled" />
        </el-form-item>
        <el-form-item :label="$t('models.cacheStrategy')">
          <el-select v-model="form.cache_strategy" :disabled="!form.cache_enabled" style="width: 200px;">
            <el-option :label="$t('models.strategyNone')" value="none" />
            <el-option :label="$t('models.strategySystem')" value="system_only" />
            <el-option :label="$t('models.strategyStandard')" value="standard" />
            <el-option :label="$t('models.strategyFull')" value="full" />
          </el-select>
          <el-tooltip placement="top">
            <template #content>
              {{ $t('models.cacheTip1') }}<br/>
              {{ $t('models.cacheTip2') }}<br/>
              {{ $t('models.cacheTip3') }}
            </template>
            <el-icon style="margin-left: 6px; cursor: help;"><QuestionFilled /></el-icon>
          </el-tooltip>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleSave" :loading="saving">{{ $t('common.save') }}</el-button>
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

const models = ref<any[]>([])
const providers = ref<any[]>([])
const loading = ref(false)
const dialogVisible = ref(false)
const isEdit = ref(false)
const saving = ref(false)
const aliasesStr = ref('')
const headerList = ref<{ key: string; value: string }[]>([])

const emptyForm = () => ({
  provider_id: '',
  vendor_model_name: '',
  display_name: '',
  aliases: [] as string[],
  model_type: 'chat',
  api_formats: ['openai'] as string[],
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
  cache_strategy: 'full',
  input_multiplier: 1.0,
  output_multiplier: 1.0,
  cache_write_multiplier: 0,
  cache_read_multiplier: 0,
  rpm_limit: 0,
  tpm_limit: 0,
  extra_headers: {} as Record<string, string>,
  enabled: true,
})
const form = ref(emptyForm())

async function fetchModels() {
  loading.value = true
  try {
    const { data } = await api.get('/models')
    models.value = data
  } catch {
    ElMessage.error(t('models.loadFailed'))
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
  isEdit.value = false
  form.value = emptyForm()
  aliasesStr.value = ''
  headerList.value = []
  dialogVisible.value = true
}

function showEdit(row: any) {
  isEdit.value = true
  form.value = { ...row }
  aliasesStr.value = (row.aliases || []).join(', ')
  const headers = row.extra_headers || {}
  headerList.value = Object.entries(headers).map(([key, value]) => ({ key, value: value as string }))
  dialogVisible.value = true
}

async function handleSave() {
  saving.value = true
  const extraHeaders: Record<string, string> = {}
  for (const h of headerList.value) {
    if (h.key.trim()) extraHeaders[h.key.trim()] = h.value
  }
  const payload = {
    ...form.value,
    aliases: aliasesStr.value ? aliasesStr.value.split(',').map(s => s.trim()).filter(Boolean) : [],
    cache_write_multiplier: form.value.cache_write_multiplier || null,
    cache_read_multiplier: form.value.cache_read_multiplier || null,
    extra_headers: Object.keys(extraHeaders).length > 0 ? extraHeaders : null,
  }
  try {
    if (isEdit.value) {
      await api.put(`/models/${form.value.provider_id}/${form.value.vendor_model_name}`, payload)
      ElMessage.success(t('models.updated'))
    } else {
      await api.post('/models', payload)
      ElMessage.success(t('models.created'))
    }
    dialogVisible.value = false
    await fetchModels()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || t('common.saveFailed'))
  } finally {
    saving.value = false
  }
}

async function handleDelete(providerId: string, modelName: string) {
  try {
    await api.delete(`/models/${providerId}/${modelName}`)
    ElMessage.success(t('models.deleted'))
    await fetchModels()
  } catch {
    ElMessage.error(t('common.deleteFailed'))
  }
}

onMounted(() => {
  fetchModels()
  fetchProviders()
})
</script>
