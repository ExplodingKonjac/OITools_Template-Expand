# texpand 架构设计文档

## 项目定位

`texpand` 是一个 C/C++ 模板展开工具，面向 Competitive Programming 场景。核心功能：将本地 `#include` 依赖递归展开为单文件，并提供语义安全的代码压缩，方便提交到 OJ 平台。

**核心挑战**：在保证对所有极端 C/C++ 语法形式绝对鲁棒的前提下，实现本地 CLI 与 VSCode 虚拟文件系统的跨端复用。

## 技术栈

| 层面 | 技术 |
|------|------|
| 核心语言 | Rust (edition 2024) |
| 语法解析 | tree-sitter + tree-sitter-cpp |
| 序列化 | serde + toml |
| CLI 框架 | clap (derive) |
| WASM 运行时 | @vscode/wasm-wasi (WASI process 模式) |
| VSCode 扩展 | TypeScript + yo code |

## Monorepo 结构

```
texpand/
├── Cargo.toml                     # workspace 定义
├── texpand-core/                  # 核心逻辑库（I/O 无关）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                 # 模块导出
│       ├── resolver.rs            # FileResolver trait
│       ├── parser.rs              # tree-sitter 封装 + include 提取
│       ├── expander.rs            # DFS 递归展开 + 预处理上下文跟踪
│       └── compressor.rs          # 语义安全代码压缩
├── texpand-cli/                   # CLI 前端
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── config.rs              # .texpand.toml 解析
└── texpand-vscode/                # VSCode 扩展前端
    ├── Cargo.toml
    ├── src/
    │   └── main.rs                # WASI 进程模式入口
    └── extension/                 # TypeScript VSCode 扩展
        ├── package.json
        ├── tsconfig.json
        └── src/
            ├── extension.ts
            └── wasm.ts            # WASM 加载封装
```

## 核心架构设计

### 1. I/O 抽象层（FileResolver）

核心库通过 trait 与 I/O 解耦，实现 CLI / VSCode 双端复用：

```rust
pub trait FileResolver {
    fn resolve(&self, includer_path: &Path, include_path: &str) -> Result<PathBuf>;
    fn read_content(&self, resolved_path: &Path) -> Result<String>;
}
```

- **CLI 端**：`FsResolver` 在配置的 `include_paths` 中按顺序搜索文件，使用 `std::fs` 读取。
- **VSCode 端**（WASM）：`WasiFsResolver` 在 WASI 沙箱内直接使用 `std::fs` 读写文件。VSCode 工作区目录被映射到 WASI 文件系统，无需 JS 桥接。

**为什么用同步方案？** WASI 进程模式下 `std::fs` 操作经过 WASI 层转发到宿主 VSCode，调用本身是阻塞的，但 `@vscode/wasm-wasi` 在扩展主线程中异步等待进程退出，不阻塞 UI。

### 2. 依赖解析与展开流程

采用 **DFS 递归展开** + **预处理上下文跟踪**：

1. **解析**：tree-sitter 解析入口文件，生成 AST。
2. **DFS 游标遍历**：使用 tree-sitter `TreeCursor` 深度优先遍历 AST：
   - 遇到 `preproc_include` 节点：
     - **Local include** → 调用 `FileResolver::resolve` 解析路径、`read_content` 读取内容，递归展开后拼入输出，跳过 children。
     - **System include** → 保留原始文本，标记为已展开（避免重复输出）。
   - 遇到 `preproc_call` 且内容为 `#pragma once` → 跳过该节点（不输出）。
   - 遇到 compound directive（`preproc_ifdef`/`preproc_ifndef`/`preproc_if`）→ 将条件 subject 压入 `PreprocContext` 栈，退出时弹出。
   - 遇到 `#else`/`#elif`/`#elifdef` → 推入 `PreprocContext`。
   - 叶子节点（非 comment）→ 直接输出原始文本。
3. **上下文感知去重**：同一文件在**不同预处理上下文**中（例如 `#ifdef X` 和 `#else` 分支）会分别展开。去重键为 `(resolved_path, PreprocContext)` 二元组。
4. **循环检测**：通过 `expanding: HashSet<PathBuf>` 跟踪当前调用栈，重复进入则报 circular dependency。
5. **系统 include**：保留 `#include <...>` 原行，不递归展开。

**为什么 DFS 而非 BFS + 拓扑排序？** 展开需要在遍历 AST 的同时按源码顺序增量输出。DFS + tree-sitter `TreeCursor` 天然支持 walk-and-emit 模式，且能无缝跟踪预处理指令的嵌套结构。原先基于 petgraph 的图展开方案因需要额外的拓扑排序和分离的展开阶段，已被内联到 DFS walk 中。

### 3. 代码压缩（Compressor）

基于 `CompressorState` 状态机的 AST 单遍压缩。核心规则：

```
遍历 AST（非叶子节点也进入，管理 preproc 状态）：

叶子节点处理：
  ├── 注释 (kind == "comment") → 丢弃
  ├── user_defined_literal（如 123_km）→ 不插空格直接拼接
  ├── #define 的 name field → 追加尾部空格（防止 FOO"bar" 合并）
  ├── 标识符相邻 (prev_last 和当前首字符均为 [a-zA-Z0-9_]) → 强制插入空格
  └── 其他情况 → 直接拼接

预处理指令保护：
  ├── 进入任何 preproc_* 节点 → 确保当前行以 \n 结尾
  ├── compound 指令 (#ifdef / #if) → 跟踪 body 起始位置，在 body 前插入 \n
  └── 离开 preproc_* 节点 → 追加 \n
```

**`#define` 名与替换文本之间**：插入空格以保证 `#define FOO"bar"` 不变成非法语法。但函数式宏（`#define FOO(x)`）检测到 `name` 后紧跟 `(` 时不插空格。

**`compress_stripped` 变体**：在单遍压缩的同时跳过 `preproc_include` 和 `#pragma once` 子树，避免二次解析。

**安全原则**：压缩优先保证语义不变，其次追求体积最小。

**为什么不用正则？** C/C++ 语法极其复杂，正则无法正确处理所有边界情况（宏嵌套、条件编译、字符串字面量中的注释等）。tree-sitter AST 是唯一可靠的方式。

### 4. 边界 Case 处理

| 场景 | 处理方式 |
|------|----------|
| 循环 `#include` | `expanding: HashSet<PathBuf>` 运行时检测 → 报 circular dependency |
| `#include <system>` | 保留原行，不入展开队列 |
| `#include "local"` | strip 原行，递归展开内容 |
| `#pragma once` | tree-sitter 匹配到 `preproc_call` 内容为 `#pragma once` → 跳过该节点 |
| 条件编译中的 include (`#ifdef` + `#include`) | 跟踪 `PreprocContext` 栈，同文件在每个条件分支中独立展开 |
| 同文件多次 include（同一上下文） | 去重键 `(resolved_path, PreprocContext)` → 仅展开一次 |
| 压缩后 `inta` 变成 `int a`？ | 不需要 — 标识符隔离规则保证 |
| 压缩后 `123_km` 变成 `123 _km`？ | 不需要 — `literal_suffix` 节点标记为 `skip_space_before` |
| `#define FOO"bar"` | 压缩器在 `name` field 后追加空格 → `#define FOO "bar"` |
| `#define FOO(x) (x)` （函数式宏） | 检测到 `name` 后紧跟 `(` → 不插空格 |
| `#include"foo.h"` 无空格 | C 预处理器接受此语法，压缩器不会额外插入空格 |
| 嵌套 preproc (`#if` 内 `#include`) | 每个 preproc 节点退出时追加 `\n` |
| 压缩后 `#define A\n#define B` 合并？ | 不会 — 每个 preproc 退出时强制 `\n` |

## 安全约束

- `texpand-core` **绝对禁止**直接调用 `std::fs` / `std::io`。所有文件读取通过 `FileResolver` trait。
- **禁止**使用正则表达式解析或匹配 C/C++ 语法（包括 include 路径提取、注释检测等）。所有语法分析必须通过 tree-sitter AST。

## 数据流全景

```
                ┌──────────────┐
                │  入口 .cpp   │
                └──────┬───────┘
                       │ source text
                       ▼
           ┌───────────────────────────┐
           │      texpand-core         │
           │                           │
           │  parse_source()           │
           │       │                   │
           │       ▼                   │
           │  tree-sitter Tree        │
           │       │                   │
           │       ▼                   │
           │  DFS TreeCursor walk  ◄──┤ ← FileResolver (trait)
           │       │                   │
           │  ┌────┴───────────┐       │
           │  │ preproc_include│       │
           │  └─┬───┘          │       │
           │    │              │       │
           │    ▼              │       │
           │  Local?           │       │
           │  ├── Yes ──→ resolve()    │
           │  │              │         │
           │  │              ▼         │
           │  │         read_content() │
           │  │              │         │
           │  │         expand_recursive│
           │  │          (DFS, stack)  │
           │  └── No ──→ 保留原行      │
           │                           │
           │  ┌──────────────────┐     │
           │  │ PreprocContext   │     │
           │  │ 栈跟踪条件分支    │     │
           │  │ (#ifdef/#if/..)  │     │
           │  └──────┬───────────┘     │
           │         │                 │
           │   去重键: (path, ctx)     │
           │                           │
           │  循环检测: expanding Set  │
           │                           │
           │  输出: ──→ 原始文本 emit  │
           │   或 ──→ CompressorState  │
           └──────────┬────────────────┘
                      │ 展开后单文件
                      ▼
             ┌────────────────┐
             │ CLI: stdout/文件 │
             │ VS: 剪贴板/新文件│
             └────────────────┘
```

## VSCode 扩展架构

通过 `@vscode/wasm-wasi` 以 **WASI 进程模式**运行 Rust 编译的 WebAssembly 模块：

```
VSCode 扩展进程                  WASM 沙箱 (texpand-vscode)
                                 (wasm32-wasip1)
┌─────────────────────────┐      ┌──────────────────────────────┐
│ extension.ts            │      │ main.rs (WASI process)      │
│                         │      │                              │
│ 1. 监听 C/C++ 文件激活   │      │ TEXPAND_ENTRY_PATH ─────→ main() │
│ 2. 构造 WASM 进程       │      │ TEXPAND_COMPRESS              │
│    (@vscode/wasm-wasi)  │      │ TEXPAND_INCLUDE_PATHS         │
│ 3. 环境变量传参          │ env  │         │                     │
│ 4. 读取 stdout          │ ←── │ WasiFsResolver                │
│                         │      │  ├── resolve()                │
│ 交互:                    │      │  └── read_content()          │
│ ├── 编辑器标题栏按钮      │      │         │                     │
│ ├── 右键上下文菜单        │      │    std::fs (via WASI)        │
│ └── 状态栏 QuickPick     │      └──────────┬───────────────────┘
│                         │                 │
│ 输出: clipboard / 新文件  │     JSON stdout {success, data?, error?}
└─────────────────────────┘      └──────────────────────────────┘
```

**关键设计**：
- 扩展依赖 `ms-vscode.wasm-wasi-core` 扩展提供 WASI 运行时。
- 参数通过环境变量传递（`TEXPAND_ENTRY_PATH`, `TEXPAND_COMPRESS`, `TEXPAND_INCLUDE_PATHS`）。
- `WasiFsResolver` 实现 `FileResolver` trait，直接使用 `std::fs` 读写（WASI 沙箱映射了工作区目录）。
- 结果以 JSON 格式写入 stdout。
- 编译目标：`wasm32-wasip1`，使用 `wasm-opt` 后处理优化。
