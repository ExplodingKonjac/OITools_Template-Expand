<div align="center">

# texpand-vscode

[English](https://github.com/ExplodingKonjac/OITools_Template-Expand/blob/main/texpand-vscode/extension/README.md) | **简体中文**

![icon](./texpand-vscode/extension/assets/icon.png)

</div>

`texpand-vscode` 是 [Template-Expand](https://github.com/ExplodingKonjac/OITools_Template-Expand) 项目的 VSCode 扩展。它能在编辑器中直接展开 C/C++ 源文件的所有本地 `#include` 依赖，将其合并为一份完整的单文件代码，并可选地进行语义安全的代码压缩。

扩展通过 WebAssembly (WASI) 运行底层的 Rust 核心逻辑，无需安装本地 CLI 工具或 Rust 工具链。

## 核心功能

- **一键展开**：通过编辑器标题栏按钮、右键菜单或命令面板，一键展开所有本地头文件依赖。
- **安全压缩**：可选地去除注释和多余空格，同时在关键词法边界保留必要空格，确保压缩后的代码语义不变。
- **零外部依赖**：扩展自带 WASM 引擎，无需额外安装任何二进制工具。
- **远程开发兼容**：通过 VSCode 虚拟文件系统工作，完全兼容 Remote-SSH、Dev Containers 和 VSCode for Web。
- **灵活输出**：支持自动复制到剪贴板，或在源文件同级目录生成 `.expanded.cpp` 文件。

## 使用方法

安装后，在任意 C/C++ 文件中可以：

1. **编辑器标题栏**：点击右上角的文件图标按钮，一键展开并复制到剪贴板。
2. **右键菜单**：右键点击编辑器，在 **Texpand** 子菜单中选择操作。
3. **命令面板**：按 `Ctrl+Shift+P` / `Cmd+Shift+P`，输入 `Texpand` 查看所有命令。
4. **状态栏**：点击右下角的 **Texpand** 按钮，快速切换压缩开关和输出模式。

## 扩展设置

在 VSCode 设置中搜索 `texpand` 即可配置：

| 设置项 | 类型 | 默认值 | 说明 |
| :--- | :--- | :--- | :--- |
| `texpand.includePaths` | `string[]` | `["./"]` | 头文件搜索路径。支持工作区相对路径或系统绝对路径。 |
| `texpand.defaultCompression` | `boolean` | `false` | 展开时是否默认开启代码压缩。 |
| `texpand.outputMode` | `"clipboard"` / `"newFile"` | `"clipboard"` | 展开结果的输出方式：复制到剪贴板，或生成新文件。 |

## 命令列表

| 命令 | 说明 |
| :--- | :--- |
| **Texpand: Expand Current File (Default)** | 根据设置的输出方式展开代码。 |
| **Texpand: Expand and Copy to Clipboard** | 展开并复制到剪贴板。 |
| **Texpand: Expand to New File** | 展开并在同级目录生成新文件。 |

## 已知问题

暂无。

## 更新日志

### 0.1.0

初始发布，支持基础展开、压缩、剪贴板与文件输出。
