# Novel Studio

Windows 优先的 Markdown 文字工作台，使用 Tauri 2、React、TypeScript 和 Vite。支持小说、产品说明书和笔记等本地项目，可编辑文档、交办段落任务，并审阅外部 harness 提交的修改建议。

## 运行

Windows 桌面构建需要 Node.js 22.12+（或 20.19+）、Rust MSVC 工具链、Visual Studio C++ Build Tools 和 WebView2。环境说明见 [Tauri 前置要求](https://v2.tauri.app/start/prerequisites/)。

```powershell
npm ci
npm run desktop
```

只看前端可运行 `npm run dev`，地址为 `http://127.0.0.1:1431`。

## 构建 Windows 可执行文件

```powershell
npm ci
npm run desktop:build
```

构建在 `src-tauri/target/release/` 生成桌面程序 `novel-studio.exe` 和 MCP 服务 `novel-studio-mcp.exe`。使用 MCP 时，把两个程序放在同一个目录。

## 使用工作台

1. 在「项目库 → 导入目录」选择现有项目目录，由 Studio 保存一份独立的托管项目。也可以用「打开项目」直接编辑一个外部目录。工作台索引 UTF-8 Markdown。
2. 在编辑器中手动修改 Markdown，点击「保存」或按 Ctrl+S。「预览」显示 Markdown 渲染效果。「重新读取」可加载外部编辑。保存时会核验文件版本。
3. 在编辑器中选中一段原文，点击「交办选区」，填写任务要求。任务保存到项目内的 `.novel-studio/tasks.json`，可随项目目录迁移。
4. 在「任务与审阅」查看 MCP 提交的建议。可以查看逐文件差异、定位原文、应用或退回。应用前会重新核验涉及文件的版本。

### 项目清单

在项目根目录内创建 `.novel-studio/project.json`。可用 [带图产品说明书](examples/product-manual) 作为起点。

- `schemaVersion` 当前为 `1`，`name` 为显示名称。
- `preset` 支持 `novel`、`manual`、`notes`、`general`。`markdown` 可选 `gfm` 或 `commonmark`，默认 GFM。
- 可选的 `documents` 按数组顺序列出优先显示的 Markdown，每项包含项目内 `path` 和可选 `kind`。角色支持 `chapter`、`summary`、`worldbuilding`、`writing`、`illustration`、`material` 和 `generated`。其他 Markdown 继续自动索引。
- Studio 会为索引到的 Markdown 登记 UUID（`documents[].id`），并保存当前目录顺序。正文章序根据清单生成，可给卷首等章节设置 `unnumbered: true` 跳过编号。

已有小说目录可直接使用。Studio 继续识别 `manuscript/` 下的章节、`manuscript/summaries/` 下的摘要和 `worldbuilding/` 下的设定。根目录的 `novel.md` 和 `word-counts.md` 沿用生成产物的只读规则。

章名通过编辑器上方的表单维护，在文件中保存为 YAML frontmatter 的 `title`。Studio 打开旧章节时会读取已有的开头一级标题，首次保存时将它移入 frontmatter 并沿用原路径。预览和 HTML 正文从 frontmatter 之后开始渲染。

## 导入和导出

导入目录会复制工作目录中的正文、设定、附件、隐藏文件和未跟踪文件。Git 历史留在来源仓库，Studio 在本机项目库管理副本。若目录包含符号链接或目录联接，先把它们替换成项目内的普通文件或目录。

「从 Git 导入」需要已安装、可从 PATH 调用的 Git。选择本地仓库根目录后，Studio 复制当前 HEAD 提交内的普通文件，在项目清单写入 `sourceCommit`，并为托管副本创建一条新的 Git 基线提交。两个提交标识可在「项目总览」查看。来源历史和未提交改动留在来源仓库。子模块和符号链接需要先转换为普通项目物料。

编辑器中的「导出 HTML」将当前文档（包括当前草稿）渲染为单个 HTML。项目内相对路径引用的 PNG、JPEG、GIF 和 WebP 会随文件嵌入，单图上限 12 MiB。项目 ZIP 用于保存当前快照，包含清单和 Git 来源标识。

保存编辑后，在「项目库 → 导出项目包」选择项目目录之外的位置保存 ZIP。项目包包含 `.novel-studio/tasks.json`，保留段落标注、任务、建议和审阅状态。用「导入项目包」打开此 ZIP，会生成一份独立的托管项目，可在项目库中切换。

## MCP 接入

打开项目后，在「MCP 接入」复制当前项目的命令和参数，填入 harness 的 stdio 服务配置。参数为 `--project` 和项目目录。harness 可独立启动服务，Studio 打开时可以查看收到的建议。[MCP 传输规范](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)

- `list_tasks` 查看待处理或待审阅的任务。
- `get_task_context` 读取所选原文、附近正文及项目规则、摘要和设定摘录。
- `search_project` 搜索项目 Markdown。`read_document` 按行读取文件并返回版本。
- `submit_proposal` 将建议送入审阅。每个现有 Markdown 文件提供一处唯一匹配的 `oldText`、`newText` 和读取时的 `version`，可同时建议修改多个文件。也可提交仅供审阅的文字意见。

## 本地存储

Windows 上的托管项目位于 `%LOCALAPPDATA%\dev.inariaimu.novel-studio\projects\<项目 UUID>\`。`workspace\` 保存项目文件，其子目录 `.novel-studio\` 保存项目清单和任务。与 `workspace\` 同级的 `project.json` 记录库名称、导入时间及可用的 Git 基线标识。从 Git 导入的项目在 `workspace\.git\` 保存本地版本库。

直接打开的外部项目在原目录读写。文件锁和中断恢复状态位于 `%LOCALAPPDATA%\novel-studio\project-runtime\`；主题和上次打开的项目保存在应用的 WebView 本地存储中。
