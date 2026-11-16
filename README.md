# Novel Studio

Windows 优先的 Tauri 2 小说工作台壳子，使用 React、TypeScript 和 Vite。界面参考 GPT Image Studio 的自绘标题栏、左侧导航与工作区布局。应用图标来自 `harness-scope/assets/icons/xai-pixel.png`。

## 运行

Windows 桌面构建需要 Node.js 22.12+（或 20.19+）、Rust MSVC 工具链、Visual Studio C++ Build Tools 和 WebView2。环境说明见 [Tauri 前置要求](https://v2.tauri.app/start/prerequisites/)。

```powershell
npm ci
npm run desktop
```

只看前端可运行 `npm run dev`，地址为 `http://127.0.0.1:1431`。用 `npm run desktop:build` 生成 `src-tauri/target/release/novel-studio.exe`。

界面使用 `src/demo.ts` 中的内置小说样例。可浏览章节、切换预览和源码、查看两类提示词、切换主题。项目导入、文件读写、通用 Markdown 渲染、模型调用和 MCP 服务待实现。当前 Rust 入口只有窗口启动逻辑。

## 功能评估

### 以 Git 仓库为项目

`E:\gitlab\the-laurea-aurea` 已具备可直接索引的结构。`manuscript/` 有卷首和 45 章正文，`manuscript/summaries/` 有章节摘要，`worldbuilding/` 有设定和章节索引，`instruction.md` 是现有写作指令。`build_novel.py` 按章校验并汇编 `novel.md`。正文优先于摘要和设定，这是该仓库自己的正典规则。

首版按文件打开纯 Markdown 仓库。以 `manuscript/*.md` 为章节候选，读取卷首、章号与标题，沿用已有排序。用可选的 `novel-studio.toml` 声明其他项目的正文、摘要、设定、提示词和生成产物路径。`novel.md` 标记为汇编产物，章节编辑入口指向 `manuscript/`。GUI 和 MCP 读同一份索引，外部 Git 切换或文件编辑后重新读取。

样本的 `novel.md` 已有约 611 KiB。按当前章、前章摘要和相关设定取材，能减少每次交给模型的文本。提示词和引用文件可随 Git 版本回溯，Codex 和 dsh 也能组装出同一套上下文。

### 章节与提示词

1. **打开项目和预览。** 先实现项目注册、章节索引、源码／预览和链接导航。验收时用 Laurea Aurea 核对卷首、45 章顺序和汇编产物识别。Markdown 预览默认禁用原始 HTML，图片和链接按项目根目录解析。
2. **写作提示词。** 把 `instruction.md` 识别为项目级约定。将按任务复用的模板放入 `prompts/writing/`，组装时显式选择当前章、摘要和相关设定，并返回引用文件，方便核验采用了哪些材料。
3. **插图提示词。** 在 `prompts/illustration/` 管理与章节或场景关联的提示词。共用模板和变量机制，保留构图、人物与视觉约束等插图专属信息。由调用方选择模型和生成流程。

### MCP 接入

项目索引与提示词组装可放入独立 Rust 核心模块，由 Tauri 命令和独立 `novel-studio-mcp` 二进制共享。让 harness 以本地 `stdio` 启动服务，传入明确的项目根目录。GUI 关闭时照常查询。首版可用 `list_chapters`、`read_chapter`、`search_project` 和 `compose_prompt` 四个只读工具跑通「找章 → 取正文与设定 → 组装写作或插图提示词」。工具入参和输出用项目相对路径定位文件，支持按小节或分页读取，控制上下文用量。[MCP 工具规范](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)

本机 Codex CLI 支持用 `codex mcp add` 注册 stdio 命令。dsh 可以在对应 profile 注册 `@deepseek-ai/dsh-mcp-client`，同样使用 stdio。本机 dsh 的 `0.1.1-rc.2` 文档只声明桥接 tools，上游文档另有资源支持，并明确暂不支持 MCP 提示词模板。把模板访问和组装放在 tools 内可覆盖这两种版本。服务实现后，以 Codex 和 dsh 分别验证工具发现和实际调用。[Codex MCP 文档](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) · [dsh MCP 插件文档](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/mcp/mcp-client/README.zh.md)

后续加写入前，先确定外部编辑冲突的处理方式。GUI 和多个 harness 都可能操作同一仓库，写操作宜先生成可审阅的 diff，并带上读取时的内容版本。应用补丁前重新校验版本，避免覆盖外部改动。第一阶段以一次真实章节预览、一次真实的写作上下文组装为验收闭环。
