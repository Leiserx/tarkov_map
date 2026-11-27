# TarkovMap — 逃离塔科夫 SVG 地图定位工具

基于 **Tauri v2 + Vue 3 + TypeScript + Rust** 构建的《逃离塔科夫》（Escape from Tarkov）轻量级桌面端地图定位工具。通过监听本地游戏截图文件名与客户端日志实现实时定位与自动切图，**纯外部文件监听、不读取游戏内存、不注入任何进程**，符合反作弊合规要求。

<img width="1282" height="842" alt="image" src="https://github.com/user-attachments/assets/9fc43675-626d-48ea-aa00-d1b359bbcf8f" />


---

## 核心功能

- **全官方地图 SVG 矢量渲染**
  - 内置 12 张 SVG 地图（海关、森林、立交桥、储备站、海岸线、塔科夫街区、灯塔、中心区、工厂、实验室、终点站、迷宫），支持纯中文名称下拉切换。
- **截图坐标与朝向实时定位**
  - 自动探测并监听 `Documents\Escape from Tarkov\Screenshots` 目录。
  - 游戏中按下 `PrintScreen` 截图后，自动解析文件名中的世界坐标 `(X, Y, Z)` 与四元数旋转，实时在地图上绘制位置红点、脉冲动效与朝向视野扇形。
- **自定义玩家昵称（防遮挡显示）**
  - 首次启动引导输入自定义名称（后续可在「设置」中修改），定位时名称会显示在红点旁，并自动置于朝向扇形的反方向（`+180°`），确保不遮挡视野指示。
- **在线 / 离线 / PVE 自动切图**
  - 监听游戏 `Logs` 目录，支持识别在线战局、离线练习模式及 PVE 模式的进图日志，自动切换至当前地图（可随时手动切换或锁定）。
- **多楼层图层与快捷切换**
  - 加载地图时默认选中地面层；多楼层地图支持左下角按钮切换或 **按住 `Ctrl` + 鼠标滚轮** 快速切换楼层。
  - 查看非地面层时保留半透明地面层作为空间参照，非当前楼层的地区标注与撤离/转移点自动半透明弱化显示。
- **中文地区名与撤离/转移点标记**
  - 内置全地图中文地区标注（立交桥已过滤纯英文/俄语标注）以及 **PMC 撤离点（绿）**、**Scav 撤离点（橙）**、**通用撤离点（蓝）**、**转移点（紫菱形）**，顶部工具栏支持一键开关显隐。
- **截图自动清理与保留数量限制**
  - 设置面板支持开启「自动清理旧截图」，并可配置「允许保留最新截图数量」（设为 `0` 表示定位后立即删除；设为 `N` 则按时间保留最新 `N` 张塔科夫截图）。
- **暗黑战术风一体化界面**
  - 采用无边框暗黑自绘标题栏与统一风格的开关控件，配置自动持久化保存至系统默认路径 `C:\Users\<用户名>\AppData\Roaming\com.tkfmap.app\settings.json`。

---

## 操作说明

| 操作 | 功能 |
| :--- | :--- |
| 鼠标左键拖拽 | 平移地图画布 |
| 鼠标滚轮 | 以鼠标指针当前位置为中心无级缩放 |
| `Ctrl` + 鼠标滚轮 | 快速切换当前地图的楼层图层 |
| 游戏内 `PrintScreen` | 触发截图并自动刷新地图位置与朝向 |

---

## 本地开发与打包

### 环境要求
- Node.js `v20+`
- Rust / Cargo `1.77+`
- Windows 10 / 11（需系统内置 WebView2 Runtime）

### 常用命令

```bash
# 安装前端依赖
npm install

# 启动桌面端开发模式
npm run tauri dev

# 构建前端产物与单文件可执行程序 (.exe)
npm run tauri build
```

打包完成后，单文件可执行程序位于 `src-tauri/target/release/tkfmap.exe`。

---

## 数据来源致谢

- 官方 SVG 地图与坐标边界参数源自 [the-hideout/tarkov-dev-svg-maps](https://github.com/the-hideout/tarkov-dev-svg-maps) 与 [tarkov.dev](https://tarkov.dev)
- 迷宫底图和截图定位参数来自 [EFTarkov 迷宫互动地图](https://www.eftarkov.com/map/6733700029c367a3d40b02af)
- 中文地区标注及撤离/转移点本地化数据整理自 [Re5pawnn/Tarkov_webmap](https://github.com/Re5pawnn/Tarkov_webmap)
