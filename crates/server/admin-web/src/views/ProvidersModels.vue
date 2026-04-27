<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">{{ $t('layout.providersModels') }}</h2>
      <el-button type="primary" @click="showCreateProvider">
        <el-icon><Plus /></el-icon> {{ $t('providers.add') }}
      </el-button>
    </div>

    <div v-loading="loading">
      <el-collapse v-model="expandedProviders">
        <el-collapse-item v-for="p in providers" :key="p.id" :name="p.id">
          <template #title>
            <div class="provider-header">
              <span class="provider-name">{{ p.display_name }}</span>
              <el-tag size="small" style="margin-left: 8px;">{{ p.vendor }}</el-tag>
              <el-tag :type="p.enabled ? 'success' : 'danger'" size="small" style="margin-left: 8px;">
                {{ p.enabled ? $t('common.enabled') : $t('common.disabled') }}
              </el-tag>
              <span class="model-count">{{ $t('providers.modelCount', { n: getProviderModels(p.id).length }) }}</span>
              <span class="provider-actions">
                <el-button size="small" @click.stop="showEditProvider(p)">{{ $t('common.edit') }}</el-button>
                <el-popconfirm :title="$t('providers.confirmDelete')" @confirm="handleDeleteProvider(p.id)">
                  <template #reference>
                    <el-button size="small" type="danger" @click.stop>{{ $t('common.delete') }}</el-button>
                  </template>
                </el-popconfirm>
              </span>
            </div>
          </template>

          <el-table :data="getProviderModels(p.id)" stripe size="small">
            <el-table-column prop="vendor_model_name" :label="$t('models.modelName')" width="220" />
            <el-table-column prop="display_name" :label="$t('models.displayName')" width="180" />
            <el-table-column prop="model_type" :label="$t('models.type')" width="100" />
            <el-table-column :label="$t('models.apiFormats')" width="160">
              <template #default="{ row }">
                <el-tag v-for="f in row.api_formats" :key="f" size="small" style="margin: 2px;">{{ f }}</el-tag>
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
            <el-table-column :label="$t('providers.health')" width="220">
              <template #default="{ row }">
                <template v-if="getHealth(row.provider_id, row.vendor_model_name)?.circuit_state === 'open'">
                  <el-tag type="danger" size="small">
                    {{ $t('providers.circuitOpen') }} ({{ $t('providers.failures', { n: getHealth(row.provider_id, row.vendor_model_name)?.consecutive_failures }) }})
                  </el-tag>
                  <el-button size="small" type="warning" style="margin-left: 4px;" @click="handleResetCircuit(row.provider_id, row.vendor_model_name)">
                    {{ $t('providers.resetCircuit') }}
                  </el-button>
                </template>
                <template v-else-if="getHealth(row.provider_id, row.vendor_model_name)?.circuit_state === 'half_open'">
                  <el-tag type="warning" size="small">{{ $t('providers.halfOpen') }}</el-tag>
                  <el-button size="small" type="warning" style="margin-left: 4px;" @click="handleResetCircuit(row.provider_id, row.vendor_model_name)">
                    {{ $t('providers.resetCircuit') }}
                  </el-button>
                </template>
                <template v-else>
                  <el-tag type="success" size="small">{{ $t('providers.healthy') }}</el-tag>
                </template>
              </template>
            </el-table-column>
            <el-table-column :label="$t('common.actions')" width="180">
              <template #default="{ row }">
                <el-button size="small" @click="showEditModel(row)">{{ $t('common.edit') }}</el-button>
                <el-popconfirm :title="$t('models.confirmDelete')" @confirm="handleDeleteModel(row.provider_id, row.vendor_model_name)">
                  <template #reference>
                    <el-button size="small" type="danger">{{ $t('common.delete') }}</el-button>
                  </template>
                </el-popconfirm>
              </template>
            </el-table-column>
          </el-table>
          <div style="margin-top: 12px;">
            <el-button @click="showCreateModel(p.id)">
              <el-icon><Plus /></el-icon> {{ $t('models.add') }}
            </el-button>
          </div>
        </el-collapse-item>
      </el-collapse>
      <el-empty v-if="!loading && providers.length === 0" />
    </div>

    <!-- Provider dialog -->
    <el-dialog v-model="providerDialogVisible" :title="isEditProvider ? $t('providers.editTitle') : $t('providers.addTitle')" width="500px">
      <el-form :model="providerForm" label-width="100px">
        <el-form-item :label="$t('common.id')" v-if="!isEditProvider">
          <el-input v-model="providerForm.id" placeholder="e.g. openai-main" />
        </el-form-item>
        <el-form-item :label="$t('providers.vendor')">
          <el-select v-model="providerForm.vendor" :placeholder="$t('providers.selectVendor')">
            <el-option label="OpenAI" value="openai" />
            <el-option label="Anthropic" value="anthropic" />
            <el-option label="DeepSeek" value="deepseek" />
            <el-option label="Custom" value="custom" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('common.name')">
          <el-input v-model="providerForm.display_name" />
        </el-form-item>
        <el-form-item :label="$t('providers.apiKey')">
          <el-input v-model="providerForm.api_key" type="password" show-password />
        </el-form-item>
        <el-form-item :label="$t('providers.authScheme')">
          <el-select v-model="providerForm.auth_scheme">
            <el-option :label="$t('providers.authSchemeAuto')" value="auto" />
            <el-option :label="$t('providers.authSchemeBearer')" value="bearer" />
            <el-option :label="$t('providers.authSchemeXApiKey')" value="x_api_key" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('providers.baseUrl')">
          <el-input v-model="providerForm.base_url" placeholder="https://api.openai.com" />
        </el-form-item>
        <el-form-item :label="$t('providers.website')">
          <el-input v-model="providerForm.website_url" placeholder="https://example.com" />
        </el-form-item>
        <el-form-item :label="$t('common.enabled')">
          <el-switch v-model="providerForm.enabled" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="providerDialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleSaveProvider" :loading="saving">{{ $t('common.save') }}</el-button>
      </template>
    </el-dialog>

    <!-- Model dialog -->
    <el-dialog v-model="modelDialogVisible" :title="isEditModel ? $t('models.editTitle') : $t('models.addTitle')" width="600px">
      <el-form :model="modelForm" label-width="140px">
        <el-form-item :label="$t('models.modelName')" v-if="!isEditModel">
          <el-input v-model="modelForm.vendor_model_name" :placeholder="$t('models.modelNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="$t('models.providerModel')" v-if="isEditModel">
          <el-input :model-value="`${modelForm.provider_id}/${modelForm.vendor_model_name}`" disabled />
        </el-form-item>
        <el-form-item :label="$t('models.displayName')">
          <el-input v-model="modelForm.display_name" />
        </el-form-item>
        <el-form-item :label="$t('models.aliases')">
          <el-input v-model="aliasesStr" :placeholder="$t('models.aliasesPlaceholder')" />
        </el-form-item>
        <el-form-item :label="$t('models.type')">
          <el-select v-model="modelForm.model_type">
            <el-option :label="$t('models.chat')" value="chat" />
            <el-option :label="$t('models.embedding')" value="embedding" />
            <el-option :label="$t('models.imageGeneration')" value="image_generation" />
            <el-option :label="$t('models.tts')" value="tts" />
            <el-option :label="$t('models.stt')" value="stt" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('models.apiFormats')">
          <el-checkbox-group v-model="modelForm.api_formats">
            <el-checkbox label="openai" value="openai">OpenAI</el-checkbox>
            <el-checkbox label="anthropic" value="anthropic">Anthropic</el-checkbox>
          </el-checkbox-group>
        </el-form-item>

        <el-divider>{{ $t('models.capabilities') }}</el-divider>
        <el-form-item :label="$t('models.features')">
          <el-checkbox v-model="modelForm.supports_streaming">{{ $t('models.streaming') }}</el-checkbox>
          <el-checkbox v-model="modelForm.supports_tools">{{ $t('models.tools') }}</el-checkbox>
          <el-checkbox v-model="modelForm.supports_vision">{{ $t('models.vision') }}</el-checkbox>
          <el-checkbox v-model="modelForm.supports_cache">{{ $t('models.cache') }}</el-checkbox>
          <el-checkbox v-model="modelForm.supports_structured_output">{{ $t('models.structuredOutput') }}</el-checkbox>
          <el-checkbox v-model="modelForm.supports_prefill">{{ $t('models.prefill') }}</el-checkbox>
          <el-checkbox v-model="modelForm.supports_web_search">{{ $t('models.webSearch') }}</el-checkbox>
        </el-form-item>
        <el-form-item :label="$t('models.contextWindow')">
          <el-input-number v-model="modelForm.context_window" :min="0" :step="1000" />
        </el-form-item>

        <el-divider>{{ $t('models.rateLimits') }}</el-divider>
        <el-form-item :label="$t('models.rpmLimit')">
          <el-input-number v-model="modelForm.rpm_limit" :min="0" :step="10" />
          <span style="margin-left: 8px; color: #909399;">{{ $t('models.unlimitedHint') }}</span>
        </el-form-item>
        <el-form-item :label="$t('models.tpmLimit')">
          <el-input-number v-model="modelForm.tpm_limit" :min="0" :step="10000" />
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
          <el-switch v-model="modelForm.enabled" />
        </el-form-item>
        <el-form-item :label="$t('models.cacheEnabled')">
          <el-switch v-model="modelForm.cache_enabled" />
        </el-form-item>
        <el-form-item :label="$t('models.cacheStrategy')">
          <el-select v-model="modelForm.cache_strategy" :disabled="!modelForm.cache_enabled" style="width: 200px;">
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
        <el-button @click="modelDialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleSaveModel" :loading="saving">{{ $t('common.save') }}</el-button>
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

interface Provider {
  id: string
  vendor: string
  display_name: string
  api_key: string
  base_url: string
  website_url: string
  auth_scheme: 'auto' | 'bearer' | 'x_api_key'
  enabled: boolean
}

interface HealthStatus {
  model_id: string
  circuit_state: string
  consecutive_failures: number
}

const providers = ref<Provider[]>([])
const models = ref<any[]>([])
const healthMap = ref<Map<string, HealthStatus>>(new Map())
const loading = ref(false)
const saving = ref(false)
const expandedProviders = ref<string[]>([])

// Provider dialog
const providerDialogVisible = ref(false)
const isEditProvider = ref(false)
const emptyProviderForm = (): Provider => ({
  id: '', vendor: 'openai', display_name: '', api_key: '', base_url: 'https://api.openai.com', website_url: '', auth_scheme: 'auto', enabled: true,
})
const providerForm = ref<Provider>(emptyProviderForm())

// Model dialog
const modelDialogVisible = ref(false)
const isEditModel = ref(false)
const aliasesStr = ref('')
const headerList = ref<{ key: string; value: string }[]>([])
const emptyModelForm = () => ({
  provider_id: '',
  vendor_model_name: '',
  display_name: '',
  aliases: [] as string[],
  model_type: 'chat',
  api_formats: ['openai'] as string[],
  supports_streaming: true,
  supports_tools: true,
  supports_structured_output: true,
  supports_vision: false,
  supports_prefill: true,
  supports_cache: true,
  supports_web_search: false,
  supports_batch: false,
  context_window: 128000,
  cache_enabled: true,
  cache_strategy: 'full',
  rpm_limit: 0,
  tpm_limit: 0,
  extra_headers: {} as Record<string, string>,
  enabled: true,
})
const modelForm = ref(emptyModelForm())

function getProviderModels(providerId: string) {
  return models.value.filter(m => m.provider_id === providerId)
}

function getHealth(providerId: string, modelName: string): HealthStatus | undefined {
  return healthMap.value.get(`${providerId}/${modelName}`)
}

async function fetchAll() {
  loading.value = true
  try {
    const [providersRes, modelsRes] = await Promise.all([
      api.get('/providers'),
      api.get('/models'),
    ])
    providers.value = providersRes.data
    models.value = modelsRes.data
    await fetchHealthStatuses()
  } catch {
    ElMessage.error(t('providers.loadFailed'))
  } finally {
    loading.value = false
  }
}

async function fetchHealthStatuses() {
  try {
    const { data } = await api.get('/health')
    const map = new Map<string, HealthStatus>()
    for (const s of data) {
      map.set(s.model_id, s)
    }
    healthMap.value = map
  } catch {
    // best-effort
  }
}

// Provider CRUD
function showCreateProvider() {
  isEditProvider.value = false
  providerForm.value = emptyProviderForm()
  providerDialogVisible.value = true
}

function showEditProvider(row: Provider) {
  isEditProvider.value = true
  providerForm.value = { ...row }
  providerDialogVisible.value = true
}

async function handleSaveProvider() {
  saving.value = true
  try {
    if (isEditProvider.value) {
      await api.put(`/providers/${providerForm.value.id}`, providerForm.value)
      ElMessage.success(t('providers.updated'))
    } else {
      await api.post('/providers', providerForm.value)
      ElMessage.success(t('providers.created'))
    }
    providerDialogVisible.value = false
    await fetchAll()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || t('common.saveFailed'))
  } finally {
    saving.value = false
  }
}

async function handleDeleteProvider(id: string) {
  try {
    await api.delete(`/providers/${id}`)
    ElMessage.success(t('providers.deleted'))
    await fetchAll()
  } catch {
    ElMessage.error(t('common.deleteFailed'))
  }
}

async function handleResetCircuit(providerId: string, modelName: string) {
  try {
    await api.post(`/health/${providerId}/${modelName}/reset`)
    ElMessage.success(t('providers.resetSuccess'))
    await fetchHealthStatuses()
  } catch {
    ElMessage.error(t('providers.resetFailed'))
  }
}

// Model CRUD
function showCreateModel(providerId: string) {
  isEditModel.value = false
  modelForm.value = { ...emptyModelForm(), provider_id: providerId }
  aliasesStr.value = ''
  headerList.value = []
  modelDialogVisible.value = true
}

function showEditModel(row: any) {
  isEditModel.value = true
  modelForm.value = { ...row }
  aliasesStr.value = (row.aliases || []).join(', ')
  const headers = row.extra_headers || {}
  headerList.value = Object.entries(headers).map(([key, value]) => ({ key, value: value as string }))
  modelDialogVisible.value = true
}

async function handleSaveModel() {
  saving.value = true
  const extraHeaders: Record<string, string> = {}
  for (const h of headerList.value) {
    if (h.key.trim()) extraHeaders[h.key.trim()] = h.value
  }
  const payload = {
    ...modelForm.value,
    aliases: aliasesStr.value ? aliasesStr.value.split(',').map(s => s.trim()).filter(Boolean) : [],
    extra_headers: Object.keys(extraHeaders).length > 0 ? extraHeaders : null,
  }
  try {
    if (isEditModel.value) {
      await api.put(`/models/${modelForm.value.provider_id}/${modelForm.value.vendor_model_name}`, payload)
      ElMessage.success(t('models.updated'))
    } else {
      await api.post('/models', payload)
      ElMessage.success(t('models.created'))
    }
    modelDialogVisible.value = false
    await fetchAll()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || t('common.saveFailed'))
  } finally {
    saving.value = false
  }
}

async function handleDeleteModel(providerId: string, modelName: string) {
  try {
    await api.delete(`/models/${providerId}/${modelName}`)
    ElMessage.success(t('models.deleted'))
    await fetchAll()
  } catch {
    ElMessage.error(t('common.deleteFailed'))
  }
}

onMounted(fetchAll)
</script>

<style scoped>
:deep(.el-collapse-item__header) {
  flex-direction: row-reverse;
}

:deep(.el-collapse-item__arrow) {
  margin-left: 0;
  margin-right: 8px;
}

.provider-header {
  display: flex;
  align-items: center;
  width: 100%;
  flex: 1;
  cursor: pointer;
}

.provider-name {
  font-weight: 600;
  font-size: 14px;
}

.model-count {
  margin-left: 12px;
  color: #909399;
  font-size: 13px;
}

.provider-actions {
  margin-left: auto;
  display: flex;
  gap: 8px;
}
</style>
