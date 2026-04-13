<template>
  <div>
    <h2 style="margin: 0 0 16px;">Usage Statistics</h2>

    <!-- Summary Cards -->
    <el-row :gutter="16" style="margin-bottom: 20px;">
      <el-col :span="4">
        <el-card shadow="hover">
          <div class="stat-label">Total Requests</div>
          <div class="stat-value">{{ summary.total_requests?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="4">
        <el-card shadow="hover">
          <div class="stat-label">Input Tokens</div>
          <div class="stat-value">{{ summary.total_input_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="4">
        <el-card shadow="hover">
          <div class="stat-label">Output Tokens</div>
          <div class="stat-value">{{ summary.total_output_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="4">
        <el-card shadow="hover">
          <div class="stat-label">Cache Write</div>
          <div class="stat-value">{{ summary.total_cache_write_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="4">
        <el-card shadow="hover">
          <div class="stat-label">Cache Read</div>
          <div class="stat-value">{{ summary.total_cache_read_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="4">
        <el-card shadow="hover">
          <div class="stat-label">Total Credits</div>
          <div class="stat-value">{{ summary.total_credits?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Filters -->
    <el-form :inline="true" style="margin-bottom: 16px;">
      <el-form-item label="API Key">
        <el-select v-model="filter.api_key_id" clearable placeholder="All keys" style="width: 200px;">
          <el-option v-for="k in apiKeys" :key="k.id" :label="k.name" :value="k.id" />
        </el-select>
      </el-form-item>
      <el-form-item label="Time Range">
        <el-date-picker
          v-model="dateRange"
          type="datetimerange"
          range-separator="-"
          start-placeholder="Start"
          end-placeholder="End"
          format="YYYY-MM-DD HH:mm"
          value-format="YYYY-MM-DDTHH:mm:ssZ"
        />
      </el-form-item>
      <el-form-item>
        <el-button type="primary" @click="fetchData">Query</el-button>
      </el-form-item>
    </el-form>

    <!-- Usage Records Table -->
    <el-table :data="records" v-loading="loading" stripe style="width: 100%;">
      <el-table-column label="Time" width="180">
        <template #default="{ row }">
          {{ formatTime(row.created_at) }}
        </template>
      </el-table-column>
      <el-table-column prop="api_key_name" label="API Key" width="150" />
      <el-table-column label="Model" min-width="200">
        <template #default="{ row }">
          {{ row.provider_id }}/{{ row.vendor_model_name }}
        </template>
      </el-table-column>
      <el-table-column prop="request_endpoint" label="Endpoint" width="180" />
      <el-table-column prop="input_tokens" label="Input" width="100" align="right">
        <template #default="{ row }">
          {{ row.input_tokens.toLocaleString() }}
        </template>
      </el-table-column>
      <el-table-column prop="output_tokens" label="Output" width="100" align="right">
        <template #default="{ row }">
          {{ row.output_tokens.toLocaleString() }}
        </template>
      </el-table-column>
      <el-table-column prop="cache_write_tokens" label="Cache W" width="100" align="right">
        <template #default="{ row }">
          {{ row.cache_write_tokens ? row.cache_write_tokens.toLocaleString() : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="cache_read_tokens" label="Cache R" width="100" align="right">
        <template #default="{ row }">
          {{ row.cache_read_tokens ? row.cache_read_tokens.toLocaleString() : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="estimated_chars" label="Est.Chars" width="110" align="right">
        <template #default="{ row }">
          {{ row.estimated_chars ? row.estimated_chars.toLocaleString() : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="credits_consumed" label="Credits" width="100" align="right">
        <template #default="{ row }">
          {{ row.credits_consumed.toLocaleString() }}
        </template>
      </el-table-column>
      <el-table-column prop="ttft_ms" label="TTFT" width="90" align="right">
        <template #default="{ row }">
          {{ row.ttft_ms > 0 ? row.ttft_ms + 'ms' : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="latency_ms" label="Latency" width="100" align="right">
        <template #default="{ row }">
          {{ row.latency_ms > 0 ? row.latency_ms + 'ms' : '-' }}
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import api from '../api'

const records = ref<any[]>([])
const summary = ref<any>({})
const apiKeys = ref<any[]>([])
const loading = ref(false)
const dateRange = ref<string[] | null>(null)

const filter = ref({
  api_key_id: '',
})

function buildQuery() {
  const q: any = {}
  if (filter.value.api_key_id) q.api_key_id = filter.value.api_key_id
  if (dateRange.value && dateRange.value.length === 2) {
    q.start_time = dateRange.value[0]
    q.end_time = dateRange.value[1]
  }
  q.limit = 200
  return q
}

async function fetchData() {
  loading.value = true
  try {
    const query = buildQuery()
    const [recordsResp, summaryResp] = await Promise.all([
      api.post('/usage', query),
      api.post('/usage/summary', query),
    ])
    records.value = recordsResp.data
    summary.value = summaryResp.data
  } catch {
    ElMessage.error('Failed to load usage data')
  } finally {
    loading.value = false
  }
}

async function fetchApiKeys() {
  try {
    const { data } = await api.get('/keys')
    apiKeys.value = data
  } catch {}
}

function formatTime(iso: string) {
  return new Date(iso).toLocaleString()
}

onMounted(() => {
  fetchData()
  fetchApiKeys()
})
</script>

<style scoped>
.stat-label {
  font-size: 13px;
  color: #909399;
  margin-bottom: 4px;
}
.stat-value {
  font-size: 24px;
  font-weight: bold;
  color: #303133;
}
</style>
