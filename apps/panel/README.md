# ProxyPanel Web

ProxyPanel Web 前端，基于 React + TypeScript + Vite + HeroUI + Tailwind CSS v4。

## 技术栈

- **框架**: React 19 + TypeScript
- **构建工具**: Vite 8 (Rolldown)
- **UI 组件库**: HeroUI
- **样式**: Tailwind CSS v4
- **路由**: React Router v7
- **国际化**: react-i18next
- **HTTP 客户端**: Axios
- **图标**: Lucide React

## 环境要求

- Node.js 20+
- npm 10+

## 可用脚本

```bash
# 安装依赖
npm install

# 启动开发服务器（带热重载）
npm run dev

# 发布构建
npm run build

# 预览发布产物
npm run preview

# TypeScript 类型检查
npx tsc --noEmit
```

## 发布构建验证

```bash
cd apps/panel
npm install
npm run build:prod
```

成功后会输出到 `dist/` 目录。Hub 默认从 `apps/panel/dist` 托管静态文件，也可通过 `--static-dir` 指定其他路径。

## 项目结构

```
src/
├── main.tsx              # React 应用入口
├── App.tsx               # 路由与布局
├── index.css             # Tailwind CSS 入口
├── api/
│   └── client.ts         # Axios 客户端与 API 封装
├── components/
│   ├── Layout.tsx        # 侧边栏 + 顶部栏布局
│   ├── nav.ts            # 导航菜单定义
│   └── ui/               # HeroUI 风格自定义组件
├── context/
│   └── AuthContext.tsx   # API Key 认证状态
├── hooks/
│   ├── useApi.ts         # 通用数据请求 Hook
│   └── usePolling.ts     # 轮询 Hook
├── i18n/
│   ├── index.ts          # i18n 初始化
│   ├── zh-CN.json        # 中文翻译
│   └── en-US.json        # 英文翻译
├── pages/                # 管理页面
│   ├── Dashboard.tsx
│   ├── Nodes.tsx
│   ├── Protocols.tsx
│   ├── Bindings.tsx
│   ├── Clients.tsx
│   ├── Groups.tsx
│   ├── Subscriptions.tsx
│   ├── Hosts.tsx
│   ├── Metrics.tsx
│   ├── Traffic.tsx
│   ├── Onlines.tsx
│   ├── Logs.tsx
│   ├── ApiKeys.tsx
│   └── Webhooks.tsx
└── utils/
    └── format.ts         # 格式化辅助函数
```

## 开发说明

- 开发服务器默认监听 `http://localhost:5173`。
- 首次访问会要求输入 Bootstrap API Key，可在 Hub 启动日志中找到。
- 前端通过相对路径 `/api/v1` 调用 Hub REST API，因此本地开发通常使用 Vite 代理或同源访问。
- 若需要自定义 Hub API 地址，可在运行开发服务器前设置环境变量：

  ```bash
  PROXYPANEL_API_URL=http://127.0.0.1:8081 npm run dev
  ```

- 构建产物输出到 `dist/`，Hub 默认从 `apps/panel/dist` 提供静态文件服务。

## 代码规范

- 使用函数组件 + React Hooks。
- 页面组件放在 `src/pages/`，可复用组件放在 `src/components/`。
- 使用 `useTranslation()` 获取翻译键，保持用户界面可国际化。
- 通过 `apiClient` 发起 HTTP 请求，错误处理统一在 Hook 中完成。
