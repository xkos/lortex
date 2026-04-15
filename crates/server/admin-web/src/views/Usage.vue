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

    <!-- Charts -->
    <el-row :gutter="16" style="margin-bottom: 20px;">
      <el-col :span="24">
        <el-card shadow="hover">
          <template #header><span>Daily Trend</span></template>
          <v-chart :option="trendOption" style="height: 300px;" autoresize />
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="16" style="margin-bottom: 20px;">
      <el-col :span="12">
        <el-card shadow="hover">
          <template #header><span>Credits by Model</span></template>
          <v-chart :option="modelOption" style="height: 300px;" autoresize />
        </el-card>
      </el-col>
      <el-col :span="12">
        <el-card shadow="hover">
          <template #header><span>Credits by API Key</span></template>
          <v-chart :option="keyOption" style="height: 300px;" autoresize />
        </el-card>
      </el-col>
    </el-row>

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
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { LineChart, PieChart, BarChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
} from 'echarts/components'
import api from '../api'

use([
  CanvasRenderer,
  LineChart,
  PieChart,
  BarChart,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
])

interface TrendPoint {
  date: string
  requests: number
  input_tokens: number
  output_tokens: number
  credits: number
}

interface GroupedUsage {
  group_key: string
  display_name: string
  requests: number
  input_tokens: number
  output_tokens: number
  credits: number
}

const records = ref<any[]>([])
const summary = ref<any>({})
const apiKeys = ref<any[]>([])
const trendData = ref<TrendPoint[]>([])
const modelData = ref<GroupedUsage[]>([])
const keyData = ref<GroupedUsage[]>([])
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
  return q
}

const trendOption = computed(() => ({
  tooltip: { trigger: 'axis' },
  legend: { data: ['Requests', 'Credits'] },
  grid: { left: 60, right: 40, bottom: 30, top: 40 },
  xAxis: { type: 'category', data: trendData.value.map((p) => p.date) },
  yAxis: [
    { type: 'value', name: 'Requests', position: 'left' },
    { type: 'value', name: 'Credits', position: 'right' },
  ],
  series: [
    {
      name: 'Requests',
      type: 'line',
      smooth: true,
      data: trendData.value.map((p) => p.requests),
    },
    {
      name: 'Credits',
      type: 'line',
      smooth: true,
      yAxisIndex: 1,
      data: trendData.value.map((p) => p.credits),
    },
  ],
}))

const modelOption = computed(() => ({
  tooltip: { trigger: 'item', formatter: '{b}: {c} ({d}%)' },
  series: [
    {
      type: 'pie',
      radius: ['35%', '65%'],
      label: { formatter: '{b}\n{d}%' },
      data: modelData.value.map((m) => ({
        name: m.display_name,
        value: m.credits,
      })),
    },
  ],
}))

const keyOption = computed(() => ({
  tooltip: { trigger: 'axis' },
  grid: { left: 100, right: 40, bottom: 30, top: 20 },
  xAxis: { type: 'value', name: 'Credits' },
  yAxis: {
    type: 'category',
    data: keyData.value.map((k) => k.display_name).reverse(),
    axisLabel: { width: 80, overflow: 'truncate' },
  },
  series: [
    {
      type: 'bar',
      data: keyData.value.map((k) => k.credits).reverse(),
    },
  ],
}))

async function fetchData() {
  loading.value = true
  try {
    const query = buildQuery()
    const [recordsResp, summaryResp, trendResp, modelResp, keyResp] =
      await Promise.all([
        api.post('/usage', { ...query, limit: 200 }),
        api.post('/usage/summary', query),
        api.post('/usage/trend', query),
        api.post('/usage/by-model', query),
        api.post('/usage/by-key', query),
      ])
    records.value = recordsResp.data
    summary.value = summaryResp.data
    trendData.value = trendResp.data
    modelData.value = modelResp.data
    keyData.value = keyResp.data
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
