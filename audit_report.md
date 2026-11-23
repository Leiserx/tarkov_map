# 代码审计报告（Audit Report）

## 1. 审计范围

- **前端（Vue 3 + TypeScript）**：`src/App.vue`、`src/components/*`、`src/composables/*`、`src/data/maps.config.ts`、`src/types/index.ts`
- **后端（Rust / Tauri v2）**：`src-tauri/src/lib.rs`、`config.rs`、`parser.rs`、`watcher.rs`、`types.rs`、`tauri.conf.json`

---

## 2. 审计维度评估

| 维度 | 评估结果 | 说明 |
| :--- | :--- | :--- |
| **功能完整性** | 通过 | 覆盖 `requirement.md` 全部 P0 需求：11 张官方地图 SVG、截图实时监听与朝向扇形、手动与日志自动切图、可选自动删除截图、普通非置顶窗口。 |
| **反作弊与系统安全** | 通过 | 后端仅通过 `notify` 监听文件系统事件与文本日志正则，无任何进程内存读取或 DLL 注入操作；自动删除仅针对已匹配塔科夫截图命名正则的 `.png` 文件。 |
| **资源与生命周期管理** | 通过 | `useTauriEvents` 在 `onUnmounted` 释放所有 IPC 监听器；`MapCanvas` 在卸载时销毁 `Panzoom` 实例；`WatcherManager::restart` 显式释放旧监听器防止线程泄漏。 |
| **编译与测试验证** | 通过 | `npm run build`（`vue-tsc --noEmit` + `vite build`）通过；`cargo test` 全部单元测试通过。 |

---

## 3. 问题分级与建议

- **P0（阻断）**：无
- **P1（当前迭代必须修复）**：无（注：开发期已发现并修复 `parser.rs` 中 `Location:\s*` 空格匹配问题及 `Woods.svg` 首次下载为空文件问题）
- **P2（可选优化）**：
  1. `MapCanvas.vue` 当前使用 `v-html` 渲染内置静态 `/maps/*.svg`，若后续扩展加载用户外部 SVG 文件，建议先通过 `DOMParser` 移除潜在 `<script>` 标签或 `on*` 事件属性。
  2. `vite build` 提示单包体积超过 500KB（主要由 `element-plus` 全量引入引起），后续可按需引入 Element Plus 组件以优化首屏加载体积。
