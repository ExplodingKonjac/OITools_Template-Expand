# texpand 架构设计文档

## 项目定位

`texpand` 是一个 C/C++ 模板展开工具，面向 Competitive Programming 场景。核心功能：将本地 `#include` 依赖递归展开为单文件，并提供语义安全的代码压缩，方便提交到 OJ 平台。

**核心挑战**：在保证对所有极端 C/C++ 语法形式绝对鲁棒的前提下，实现本地 CLI 与 VSCode 虚拟文件系统的跨端复用。

## 技术栈

| 层面 | 技术 |
|------|------|
| 核心语言 | Rust (edition 2024) |
| 语法解析 | tree-sitter + tree-sitter-cpp |
| 图算法 | petgraph (Tarjan SCC) |
| 序列化 | serde + toml |
| CLI 框架 | clap (derive) |
| WASM 绑定 | wasm-bindgen |
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
│       ├── graph.rs               # 依赖图 + Tarjan 环路检测
│       ├── expander.rs            # BFS 展开编排
│       └── compressor.rs          # 语义安全代码压缩
├── texpand-cli/                   # CLI 前端
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── config.rs              # .texpand.toml 解析
└── texpand-vscode/                # VSCode 扩展前端
    ├── Cargo.toml
    ├── src/
    │   └── lib.rs                 # wasm-bindgen 入口
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
    fn resolve_and_read(&self, include_path: &str) -> Result<(String, String)>;
}
```

- **CLI 端**：`FsResolver` 在配置的 `include_paths` 中按顺序搜索文件，使用 `std::fs` 读取。
- **VSCode 端**（WASM）：JS 侧预先读取工作区所有文件，一次性以 `HashMap<String, String>` 传入 Rust WASM 模块。`FileResolver` 只做内存查找。

**为什么用同步方案？** WASM 端 JS 调用 `vscode.workspace.fs.readFile` 虽然是异步的，但我们可以预读所有文件，Rust 侧使用同步查找以避免异步桥接的复杂度。

### 2. 依赖解析与展开流程

```
entry.cpp → parse_source()
              ↓
         extract_all_includes() → [Include::Local, Include::System]
              ↓                     ↓                    ↓
         BFS queue          resolve via          add to graph,
                            FileResolver         keep in output
              ↓
         DependencyGraph (petgraph)
              ↓
         Tarjan SCC 环检测 → 有环则报错
              ↓
         拓扑排序（反序）→ 依赖在前
              ↓
         拼接输出（strip local #include, 保留 system #include）
```

关键步骤：
1. **解析**：tree-sitter 解析 C/C++ 源码，生成 AST。
2. **分类**：遍历 AST 找到 `preproc_include` 节点，区分 `Local("...")` 和 `System(<...>)`。
3. **BFS 发现**：用 BFS 队列逐层发现所有本地依赖文件。
4. **建图**：`includer → includee` 有向边构建依赖图。
5. **环检测**：Tarjan 算法求 SCC，任何 size > 1 的 SCC 即为环。
6. **展开顺序**：反转拓扑排序结果 — 叶子依赖先输出，入口文件最后输出。
7. **拼接**：
   - 本地 `#include "..."` → 从源码中 strip（内容已展开）
   - 系统 `#include <...>` → 保留原行

### 3. 代码压缩（Compressor）

基于 tree-sitter AST 叶节点（token）的状态机压缩：

```
遍历 AST 叶子节点：
  ├── 注释 (kind == "comment") → 丢弃
  ├── 标识符相邻 (前后 char 均为 [a-zA-Z0-9_]) → 强制插入空格
  └── 其他情况 → 直接拼接，不加空格

预处理指令保护：
  └── 离开 preproc_* 节点时 → 强制追加换行符 \n
```

**安全原则**：压缩优先保证语义不变，其次追求体积最小。

**为什么不用正则？** C/C++ 语法极其复杂，正则无法正确处理所有边界情况（宏嵌套、条件编译、字符串字面量中的注释等）。tree-sitter AST 是唯一可靠的方式。

### 4. 边界 Case 处理

| 场景 | 处理方式 |
|------|----------|
| 循环 `#include` | Tarjan SCC 检测 → 提取环路径 → 致命错误 |
| `#include <system>` | 保留原行，不入展开队列 |
| `#include "local"` | strip 原行，递归展开内容 |
| 条件编译中的 include (`#ifdef`) | tree-sitter 仍然解析出 `preproc_include` 节点，正常处理 |
| 宏展开中的 include | tree-sitter 无法解析宏展开后的 include（预处理阶段）、但会保留宏定义本身 |
| 压缩后 `inta` 变成 `int a`？ | 不需要 — 标识符隔离规则保证 `int` 和 `a` 之间维持空格 |
| `#include"foo.h"` 无空格 | C 预处理器接受此语法，压缩器不会额外插入空格 |
| 嵌套 preproc (`#if` 内 `#include`) | 每个 preproc 节点退出时追加 `\n`，保证预处理指令完整性 |

## 安全约束

- `texpand-core` **绝对禁止**直接调用 `std::fs` / `std::io`。所有文件读取通过 `FileResolver` trait。
- tree-sitter **禁止**正则表达式替代（`#[deny(unsafe_regex)]` 策略 — 通过 clippy 禁止不安全的正则使用，而非通过 unsafe 规则）。

## 数据流全景

```
                ┌──────────────┐
                │  入口 .cpp   │
                └──────┬───────┘
                       │ source text
                       ▼
           ┌───────────────────────┐
           │    texpand-core       │
           │                       │
           │  parse_source() ──→ Tree
           │       │               │
           │       ▼               ▼
           │  extract_all_includes │
           │       │               │
           │  ┌────┴──────────┐    │
           │  │ Local  System  │    │
           │  └─┬───┘    │    │    │
           │    │        │    │    │
           │    ▼        │    ▼    │
           │ resolve()   │  keep   │
           │ via trait   │  as-is  │
           │    │        │    │    │
           │    ▼        │    │    │
           │ BFS queue ──┘    │    │
           │    │             │    │
           │    ▼             │    │
           │ DependencyGraph  │    │
           │    │             │    │
           │    ▼             │    │
           │ expansion_order()│    │
           │    │             │    │
           │    ▼             ▼    │
           │  expand()/compress()  │
           └───────┬───────────────┘
                   │ 展开后单文件
                   ▼
          ┌────────────────┐
          │ CLI: stdout/文件 │
          │ VS: 剪贴板/新文件│
          └────────────────┘
```

## VSCode 扩展架构

```
VSCode 扩展进程                  WASM 沙箱
┌─────────────────┐          ┌──────────────────┐
│ extension.ts    │          │ texpand_vscode   │
│                 │          │ (wasm-bindgen)   │
│ 1. 读取工作区    │          │                  │
│    所有文件      │  JSON    │ expand(source,   │
│ 2. 构造文件 Map  │ ───────→ │ files, config)   │
│ 3. 调用 WASM     │          │       │          │
│ 4. 写剪贴板/文件 │ ←─────── │ expanded text   │
└─────────────────┘          └──────────────────┘
```
