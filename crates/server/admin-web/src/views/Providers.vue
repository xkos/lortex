<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">{{ $t('providers.title') }}</h2>
      <el-button type="primary" @click="showCreate">
        <el-icon><Plus /></el-icon> {{ $t('providers.add') }}
      </el-button>
    </div>

    <el-table :data="providers" v-loading="loading" stripe>
      <el-table-column prop="id" :label="$t('common.id')" width="180" />
      <el-table-column prop="vendor" :label="$t('providers.vendor')" width="120" />
      <el-table-column prop="display_name" :label="$t('common.name')" />
      <el-table-column prop="base_url" :label="$t('providers.baseUrl')" />
      <el-table-column prop="website_url" :label="$t('providers.website')" width="160">
        <template #default="{ row }">
          <a v-if="row.website_url" :href="row.website_url" target="_blank" rel="noopener">{{ row.website_url.replace(/^https?:\/\//, '') }}</a>
          <span v-else>-</span>
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
          <el-popconfirm :title="$t('providers.confirmDelete')" @confirm="handleDelete(row.id)">
            <template #reference>
              <el-button size="small" type="danger">{{ $t('common.delete') }}</el-button>
            </template>
          </el-popconfirm>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialogVisible" :title="isEdit ? $t('providers.editTitle') : $t('providers.addTitle')" width="500px">
      <el-form :model="form" label-width="100px">
        <el-form-item :label="$t('common.id')" v-if="!isEdit">
          <el-input v-model="form.id" placeholder="e.g. openai-main" />
        </el-form-item>
        <el-form-item :label="$t('providers.vendor')">
          <el-select v-model="form.vendor" :placeholder="$t('providers.selectVendor')">
            <el-option label="OpenAI" value="openai" />
            <el-option label="Anthropic" value="anthropic" />
            <el-option label="DeepSeek" value="deepseek" />
            <el-option label="Custom" value="custom" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('common.name')">
          <el-input v-model="form.display_name" />
        </el-form-item>
        <el-form-item :label="$t('providers.apiKey')">
          <el-input v-model="form.api_key" type="password" show-password />
        </el-form-item>
        <el-form-item :label="$t('providers.baseUrl')">
          <el-input v-model="form.base_url" placeholder="https://api.openai.com/v1" />
        </el-form-item>
        <el-form-item :label="$t('providers.website')">
          <el-input v-model="form.website_url" placeholder="https://example.com" />
        </el-form-item>
        <el-form-item :label="$t('common.enabled')">
          <el-switch v-model="form.enabled" />
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
  enabled: boolean
}

const providers = ref<Provider[]>([])
const loading = ref(false)
const dialogVisible = ref(false)
const isEdit = ref(false)
const saving = ref(false)

const emptyForm = (): Provider => ({
  id: '', vendor: 'openai', display_name: '', api_key: '', base_url: 'https://api.openai.com/v1', website_url: '', enabled: true,
})
const form = ref<Provider>(emptyForm())

async function fetchProviders() {
  loading.value = true
  try {
    const { data } = await api.get('/providers')
    providers.value = data
  } catch (e: any) {
    ElMessage.error(t('providers.loadFailed'))
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
      ElMessage.success(t('providers.updated'))
    } else {
      await api.post('/providers', form.value)
      ElMessage.success(t('providers.created'))
    }
    dialogVisible.value = false
    await fetchProviders()
  } catch (e: any) {
    ElMessage.error(e.response?.data?.error?.message || t('common.saveFailed'))
  } finally {
    saving.value = false
  }
}

async function handleDelete(id: string) {
  try {
    await api.delete(`/providers/${id}`)
    ElMessage.success(t('providers.deleted'))
    await fetchProviders()
  } catch (e: any) {
    ElMessage.error(t('common.deleteFailed'))
  }
}

onMounted(fetchProviders)
</script>
