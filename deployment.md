# 部署与运行指南（Deployment Guide）

## 1. 环境要求

- **Node.js**：`v20+`（当前验证环境为 `v22.12.0`）
- **Rust / Cargo**：`1.77+`（当前验证环境为 `1.100.0`）
- **Windows 系统依赖**：WebView2 Runtime（Windows 10/11 通常已内置），以及 MSVC 构建工具链（Visual Studio C++ Build Tools，仅打包编译时需要）

## 2. 安装依赖

在项目根目录执行：

```bash
npm install
```

## 3. 本地开发运行

启动 Tauri 开发模式（同时启动 Vite 前端热更新与 Rust 桌面窗口）：

```bash
npm run tauri dev
```

## 4. 生产打包

构建 Windows 安装包（`.msi` / `.exe`）：

```bash
npm run tauri build
```

打包产物位于 `src-tauri/target/release/bundle/` 目录下。

## 5. 地图资源说明

- 本项目内置全官方地图 SVG 资源位于 `public/maps/`，元数据与校准参数位于 `src/data/maps.config.ts`（源自 `tarkov.dev` 开源地图项目）。
- 软件默认自动探测 `%USERPROFILE%\Documents\Escape from Tarkov\Screenshots` 作为截图监听目录；若游戏安装或系统文档目录有调整，可在软件右上角「设置」中手动指定。
