# Template-Expand

Template-Expand 是一个专为 C/C++ Competitive Programming 设计的本地模板自动展开工具。包含 VSCode 扩展和本地 CLI 工具 `texpand`。

它能够精确解析 C/C++ 代码，自动展开本地依赖的头文件，同时提供语义安全的 token 级别代码压缩功能，方便选手快速复制并提交合并后的单文件代码。

## 核心特性

- **精确解析**：基于 Tree-sitter 增量解析生成 AST。
- **循环依赖处理**：将头文件包含关系构建为图，自动检测循环包含并报错，按正确的拓扑排序拼接代码。
- **安全压缩**：提供可选的代码压缩功能。通过遍历 AST 叶子节点，仅去除注释和多余空格，并在关键词法边界（如标识符之间）保留必要空格，确保压缩后的代码语义不发生任何改变。
- **跨平台双端支持**：提供本地命令行工具与 VSCode 扩展两种使用形态，两端共享同一套底层 Rust 解析逻辑。

## 基础架构概览

项目采用 monorepo 架构：
- **`texpand-core`**：底层核心处理库，纯计算模块，负责解析、构建依赖图和代码压缩。
- **`texpand-cli`**：命令行前端，负责读取本地文件系统。
- **`texpand-vscode`**：编辑器前端，通过 WebAssembly 调用核心库，读取编辑器工作区文件。

## 配置文件文档

在用户配置目录 `~/.config/` 目录下创建 `texpand.toml` 进行自定义配置，使用 TOML 格式。

```toml
# 本地头文件搜索路径列表，按顺序查找
include_paths = [
    "./templates",
    "~/algo/cpp_lib"
]

# 默认是否开启代码压缩
default_compress = false
```

## 命令行工具文档

### 安装

确保已安装 Rust 工具链，然后在项目根目录执行：

```bash
cargo install --path texpand-cli
```

### 基本用法

```bash
# 展开 main.cpp 并将结果输出到标准输出
texpand main.cpp

# 展开 main.cpp，开启代码压缩，并保存到 output.cpp
texpand main.cpp -c -o output.cpp

# 展开 main.cpp，使用自定义头文件搜索路径
texpand main.cpp -i ./templates -i ~/cp-lib

# 展开 main.cpp，开启压缩，并将结果复制到剪贴板
texpand main.cpp -c -C

# 从标准输入读取源代码
cat main.cpp | texpand - -c

# 使用自定义配置文件
texpand main.cpp --config /path/to/my-config.toml
```

### 参数说明

- `[INPUT]`：必填，需要展开的 C/C++ 源文件路径。传 `-` 表示从标准输入 (stdin) 读取。
- `-c, --compress`：开启 Token 级别的代码压缩（去除注释和无用空格）。会覆盖配置文件中的 `default_compress`。
- `--no-compress`：禁用代码压缩。会覆盖配置文件中的 `default_compress`。
- `-i, --include <PATH>`：可重复使用，添加头文件搜索路径。指定了任何 `-i` 后，配置文件中的 `include_paths` 将被忽略。
- `-o, --output <FILE>`：将展开结果输出到指定文件。不指定则输出到标准输出。
- `-C, --clipboard`：将展开结果复制到系统剪贴板（跨平台：Windows/macOS/Linux）。
- `--config <FILE>`：指定配置文件路径。默认读取 `~/.config/texpand.toml`。

## VSCode 扩展文档

`texpand-vscode` 是 `texpand` 项目的编辑器前端扩展。它通过 WebAssembly (WASM) 直接运行底层的 Rust 核心解析逻辑，在 VSCode 中提供一键展开 C/C++ 本地模板、安全压缩代码并输出到剪贴板的功能。该扩展在独立的虚拟文件系统中运行，完全不依赖本地的 `texpand-cli` 可执行文件。

以下是用于 `texpand-vscode` 子目录的详细扩展开发与使用文档。

### 核心功能

- **右键/快捷键触发**：在打开的 C/C++ 源文件中，通过命令面板或右键菜单一键展开当前代码的所有本地 `#include` 依赖。
- **零外部依赖**：无需在操作系统中配置 Rust 环境或安装额外的二进制 CLI 工具，扩展自带 WASM 核心引擎。
- **虚拟文件系统兼容**：直接调用 VSCode 的 `workspace.fs` API 读取文件，完全兼容远程开发 (Remote-SSH) 和 Web 端工作区 (VSCode for Web)。
- **智能剪贴板注入**：展开及压缩完成后，自动将最终的单文件代码写入系统剪贴板，可直接粘贴至各大 OJ (Online Judge) 平台。

### 扩展配置项

可以在 VSCode 的 `settings.json` 中搜索 `texpand` 进行自定义配置。这些配置会在执行命令时动态传递给 WASM 核心。

| 配置项键名 | 类型 | 默认值 | 作用说明 |
| :--- | :--- | :--- | :--- |
| `texpand.includePaths` | `string[]` | `["./"]` | 本地模板头文件的搜索路径。支持相对于当前工作区根目录的相对路径，或系统的绝对路径。 |
| `texpand.defaultCompression` | `boolean` | `false` | 展开代码时，是否默认开启去注释、去多余空格的 Token 级安全压缩。 |
| `texpand.outputMode` | `enum` | `"clipboard"` | 结果输出方式。可选值：`"clipboard"` (复制到剪贴板)、`"newFile"` (在当前目录下生成 `.expanded.cpp` 文件)。 |

### 注册命令 (Commands)

扩展向命令面板 (`Ctrl+Shift+P` / `Cmd+Shift+P`) 注册了以下指令：

1. **Texpand: Expand Current File (Default)**
    使用设置中的 `texpand.outputMode` 决定输出去向。
2. **Texpand: Expand and Copy to Clipboard**
    强制展开并复制到剪贴板。
3. **Texpand: Expand to New File**
    强制展开并在同级目录生成新文件。

### 技术架构与本地构建指南

本节面向扩展的二次开发者。
