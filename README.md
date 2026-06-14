# Paper Vault Core

一个本地论文阅读与整理工具的核心精简版，基于 `Tauri + React + TypeScript + SQLite`。

这个仓库保留了项目的核心框架和主要组件，目标是：

- 克隆后可以直接安装依赖并启动
- 保留论文导入、元数据管理、PDF 阅读、Markdown 笔记、AI 处理的核心代码
- 去掉本地运行产物、测试数据、缓存和个人工作目录

## 当前包含的核心能力

- 本地 PDF 导入与存储
- 论文条目与元数据管理
- PDF 阅读
- 阅读进度记录
- Markdown 笔记
- 全文搜索
- 重复检测
- BibTeX / Markdown 引用导出
- AI 元数据提取、翻译、总结、创建笔记预设

## 环境要求

Windows 环境下建议先准备：

- Node.js 20+
- `pnpm`（通过 `corepack` 使用）
- Rust 工具链
- Visual Studio 2022 Build Tools（含 MSVC）
- Windows SDK

## 快速开始

### 1. 安装前端依赖

```bash
corepack enable
pnpm install
```

### 2. 启动开发版

直接运行：

```bat
run-dev.bat
```

该脚本会：

- 初始化 MSVC 编译环境
- 注入 Cargo 路径
- 使用项目内 `.tmp` 目录作为临时目录
- 启动 `pnpm tauri dev`

### 3. 常用命令

```bash
pnpm typecheck
pnpm test
pnpm tauri dev
pnpm tauri build
```

## 目录说明

```text
src/         前端界面与状态管理
src-tauri/   Tauri 后端、数据库、PDF、AI、导出能力
run-dev.bat  Windows 开发启动脚本
```

## 说明

- 这是整理后的核心仓库，不包含你的本地论文库、数据库、导出结果和缓存文件。
- 第一次运行时，应用会在本地创建自己的数据目录。
- AI 功能依赖你自己配置兼容的 API 提供方。
