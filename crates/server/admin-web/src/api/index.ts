import axios from 'axios'

const api = axios.create({
  baseURL: '/admin/api/v1',
  timeout: 10000,
})

// 请求拦截：自动附加 admin key
api.interceptors.request.use((config) => {
  const key = sessionStorage.getItem('admin_key')
  if (key) {
    config.headers.Authorization = `Bearer ${key}`
  }
  return config
})

// 响应拦截：401 跳转登录
api.interceptors.response.use(
  (resp) => resp,
  (error) => {
    if (error.response?.status === 401) {
      sessionStorage.removeItem('admin_key')
      window.location.hash = '#/login'
    }
    return Promise.reject(error)
  }
)

export default api
