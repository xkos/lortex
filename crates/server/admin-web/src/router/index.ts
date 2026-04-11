import { createRouter, createWebHashHistory } from 'vue-router'

const routes = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('../views/Login.vue'),
  },
  {
    path: '/',
    name: 'Layout',
    component: () => import('../views/Layout.vue'),
    redirect: '/providers',
    children: [
      {
        path: 'providers',
        name: 'Providers',
        component: () => import('../views/Providers.vue'),
      },
      {
        path: 'models',
        name: 'Models',
        component: () => import('../views/Models.vue'),
      },
      {
        path: 'keys',
        name: 'ApiKeys',
        component: () => import('../views/ApiKeys.vue'),
      },
    ],
  },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

// 路由守卫：未登录跳转到登录页
router.beforeEach((to, _from, next) => {
  const key = sessionStorage.getItem('admin_key')
  if (to.name !== 'Login' && !key) {
    next({ name: 'Login' })
  } else {
    next()
  }
})

export default router
