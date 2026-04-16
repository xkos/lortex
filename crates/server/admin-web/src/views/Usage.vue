<template>
  <div>
    <h2 style="margin: 0 0 16px;">{{ $t('usage.title') }}</h2>

    <!-- Summary Cards — Row 1: core metrics -->
    <el-row :gutter="16" style="margin-bottom: 12px;">
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-label">{{ $t('usage.totalRequests') }}</div>
          <div class="stat-value">{{ summary.total_requests?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-label">{{ $t('usage.inputTokens') }}</div>
          <div class="stat-value">{{ summary.total_input_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-label">{{ $t('usage.outputTokens') }}</div>
          <div class="stat-value">{{ summary.total_output_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-label">{{ $t('usage.totalCredits') }}</div>
          <div class="stat-value">{{ summary.total_credits?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
    </el-row>
    <!-- Summary Cards — Row 2: cache metrics -->
    <el-row :gutter="16" style="margin-bottom: 20px;">
      <el-col :span="8">
        <el-card shadow="hover">
          <div class="stat-label">{{ $t('usage.cacheWrite') }}</div>
          <div class="stat-value">{{ summary.total_cache_write_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card shadow="hover">
          <div class="stat-label">{{ $t('usage.cacheRead') }}</div>
          <div class="stat-value">{{ summary.total_cache_read_tokens?.toLocaleString() || 0 }}</div>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card shadow="hover">
          <div class="stat-label">{{ $t('usage.cacheHitRate') }}</div>
          <div class="stat-value">{{ cacheHitRate }}%</div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Filters -->
    <el-form :inline="true" style="margin-bottom: 16px;">
      <el-form-item :label="$t('usage.apiKey')">
        <el-select v-model="filter.api_key_id" clearable :placeholder="$t('usage.allKeys')" style="width: 200px;">
          <el-option v-for="k in apiKeys" :key="k.id" :label="k.name" :value="k.id" />
        </el-select>
      </el-form-item>
      <el-form-item :label="$t('usage.timeRange')">
        <el-date-picker
          v-model="dateRange"
          type="datetimerange"
          range-separator="-"
          :start-placeholder="$t('usage.start')"
          :end-placeholder="$t('usage.end')"
          format="YYYY-MM-DD HH:mm"
          value-format="YYYY-MM-DDTHH:mm:ssZ"
        />
      </el-form-item>
      <el-form-item>
        <el-button type="primary" @click="fetchData">{{ $t('usage.query') }}</el-button>
      </el-form-item>
    </el-form>

    <!-- Charts -->
    <el-row :gutter="16" style="margin-bottom: 20px;">
      <el-col :span="24">
        <el-card shadow="hover">
          <template #header><span>{{ $t('usage.dailyTrend') }}</span></template>
          <v-chart :option="trendOption" style="height: 300px;" autoresize />
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="16" style="margin-bottom: 20px;">
      <el-col :span="12">
        <el-card shadow="hover">
          <template #header><span>{{ $t('usage.creditsByModel') }}</span></template>
          <v-chart :option="modelOption" style="height: 300px;" autoresize />
        </el-card>
      </el-col>
      <el-col :span="12">
        <el-card shadow="hover">
          <template #header><span>{{ $t('usage.creditsByKey') }}</span></template>
          <v-chart :option="keyOption" style="height: 300px;" autoresize />
        </el-card>
      </el-col>
    </el-row>

    <!-- Usage Records Table -->
    <el-table :data="records" v-loading="loading" stripe style="width: 100%;">
      <el-table-column :label="$t('usage.time')" width="180">
        <template #default="{ row }">
          {{ formatTime(row.created_at) }}
        </template>
      </el-table-column>
      <el-table-column prop="api_key_name" :label="$t('usage.apiKey')" width="150" />
      <el-table-column :label="$t('usage.model')" min-width="200">
        <template #default="{ row }">
          {{ row.provider_id }}/{{ row.vendor_model_name }}
        </template>
      </el-table-column>
      <el-table-column prop="request_endpoint" :label="$t('usage.endpoint')" width="180" />
      <el-table-column prop="input_tokens" :label="$t('usage.input')" width="100" align="right">
        <template #default="{ row }">
          {{ row.input_tokens.toLocaleString() }}
        </template>
      </el-table-column>
      <el-table-column prop="output_tokens" :label="$t('usage.output')" width="100" align="right">
        <template #default="{ row }">
          {{ row.output_tokens.toLocaleString() }}
        </template>
      </el-table-column>
      <el-table-column prop="cache_write_tokens" :label="$t('usage.cacheW')" width="100" align="right">
        <template #default="{ row }">
          {{ row.cache_write_tokens ? row.cache_write_tokens.toLocaleString() : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="cache_read_tokens" :label="$t('usage.cacheR')" width="100" align="right">
        <template #default="{ row }">
          {{ row.cache_read_tokens ? row.cache_read_tokens.toLocaleString() : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="estimated_chars" :label="$t('usage.estChars')" width="110" align="right">
        <template #default="{ row }">
          {{ row.estimated_chars ? row.estimated_chars.toLocaleString() : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="credits_consumed" :label="$t('usage.credits')" width="100" align="right">
        <template #default="{ row }">
          {{ row.credits_consumed.toLocaleString() }}
        </template>
      </el-table-column>
      <el-table-column prop="ttft_ms" :label="$t('usage.ttft')" width="90" align="right">
        <template #default="{ row }">
          {{ row.ttft_ms > 0 ? row.ttft_ms + 'ms' : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="latency_ms" :label="$t('usage.latency')" width="100" align="right">
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
import { useI18n } from 'vue-i18n'
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

const { t } = useI18n()

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
  cache_write_tokens: number
  cache_read_tokens: number
  credits: number
}

interface GroupedUsage {
  group_key: string
  display_name: string
  requests: number
  input_tokens: number
  output_tokens: number
  cache_write_tokens: number
  cache_read_tokens: number
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

const cacheHitRate = computed(() => {
  const read = summary.value.total_cache_read_tokens || 0
  const write = summary.value.total_cache_write_tokens || 0
  const total = read + write
  return total > 0 ? (read / total * 100).toFixed(1) : '0.0'
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
  legend: { data: [t('usage.requests'), t('usage.credits'), t('usage.cacheHitRate')] },
  grid: { left: 60, right: 100, bottom: 30, top: 40 },
  xAxis: { type: 'category', data: trendData.value.map((p) => p.date) },
  yAxis: [
    { type: 'value', name: t('usage.requests'), position: 'left' },
    { type: 'value', name: t('usage.credits'), position: 'right' },
    { type: 'value', name: t('usage.cacheHitRate'), position: 'right', offset: 50, min: 0, max: 100, axisLabel: { formatter: '{value}%' } },
  ],
  series: [
    {
      name: t('usage.requests'),
      type: 'line',
      smooth: true,
      data: trendData.value.map((p) => p.requests),
    },
    {
      name: t('usage.credits'),
      type: 'line',
      smooth: true,
      yAxisIndex: 1,
      data: trendData.value.map((p) => p.credits),
    },
    {
      name: t('usage.cacheHitRate'),
      type: 'line',
      smooth: true,
      yAxisIndex: 2,
      lineStyle: { type: 'dashed' },
      data: trendData.value.map((p) => {
        const total = (p.cache_read_tokens || 0) + (p.cache_write_tokens || 0)
        return total > 0 ? Number(((p.cache_read_tokens || 0) / total * 100).toFixed(1)) : 0
      }),
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
  xAxis: { type: 'value', name: t('usage.credits') },
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
    ElMessage.error(t('usage.loadFailed'))
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
