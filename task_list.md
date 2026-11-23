# 任务清单（Task List）

状态标记说明：`- [ ]` 待开发 | `- [x]` 已开发 | `- [-]` 跳过

## 1. 工程初始化与地图资源准备
- [x] T1.1 检查本地环境（Node / pnpm / Rust / Cargo）并初始化 Tauri v2 + Vue 3 + TypeScript + Vite 工程（使用 `npm` 完成包管理）
- [x] T1.2 安装前端依赖（`element-plus`、`@element-plus/icons-vue`、`@panzoom/panzoom`、`@tauri-apps/api`、`@tauri-apps/plugin-dialog`）与 Rust 依赖（`notify`、`regex`、`serde`、`serde_json`、`dirs`、`tauri-plugin-dialog`）
- [x] T1.3 从 `tarkov.dev` 获取全官方地图元数据（`bounds`、`coordinateRotation`）及 11 张 SVG 地图文件，生成 `src/data/maps.config.ts`

## 2. Rust 后端核心模块开发
- [x] T2.1 实现数据结构与配置管理模块（`src-tauri/src/types.rs`、`src-tauri/src/config.rs`），支持自动探测默认截图/日志路径与持久化 `settings.json`
- [x] T2.2 实现截图文件名正则解析模块（`src-tauri/src/parser.rs`），提取 `(X, Y, Z)` 与四元数 `(qX, qY, qZ, qW)` 并编写单元测试
- [x] T2.3 实现文件与日志监听模块（`src-tauri/src/watcher.rs`），使用 `notify` 监听截图创建（支持可选自动删除）及游戏日志切图识别
- [x] T2.4 注册 Tauri Commands 与配置非置顶常规桌面窗口（`src-tauri/src/lib.rs`、`src-tauri/tauri.conf.json`）

## 3. 前端核心模块与 UI 实现
- [x] T3.1 配置暗黑战术风全局样式与 Element Plus 暗黑主题（`src/assets/styles/theme.css`、`src/main.ts`）
- [x] T3.2 实现坐标转换引擎与四元数偏航角计算（`src/composables/useMapTransform.ts`）
- [x] T3.3 实现 Tauri IPC 事件封装（`src/composables/useTauriEvents.ts`）
- [x] T3.4 实现顶部紧凑工具栏组件（`src/components/TopToolbar.vue`）
- [x] T3.5 实现全画幅 SVG 地图画布与玩家定位/朝向扇形组件（`src/components/MapCanvas.vue`）
- [x] T3.6 实现右下角悬浮控制条组件（`src/components/MapControls.vue`）
- [x] T3.7 实现设置弹窗组件（`src/components/SettingsModal.vue`）并集成至 `src/App.vue`

## 4. 验证、审计与部署文档
- [x] T4.1 运行前端类型检查/构建（`npm run build`）与 Rust 编译测试（`cargo test`）
- [x] T4.2 编写部署与使用说明文档（`deployment.md`）
- [x] T4.3 执行 Phase 5 代码审计并输出 `audit_report.md`
