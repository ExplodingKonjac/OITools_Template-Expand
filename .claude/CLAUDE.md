## 项目定位与目标

`texpand` 是一个 C/C++ 模板展开工具。其核心挑战在于：**在保证对所有极端 C/C++ 语法形式绝对鲁棒的前提下，实现本地代码与 VSCode 虚拟文件系统的跨端复用。**

## 核心技术栈

- **开发语言**：Rust (Workspaces / Monorepo 组织形式)
- **语法解析**：`tree-sitter` & `tree-sitter-cpp`（绝对禁止使用正则表达式匹配 C++ 语法）
- **图算法**：`petgraph`（用于依赖路径解析和环路检测）
- **序列化**：`serde` + `toml`
- **WASM 绑定**：`wasm-bindgen`（用于编译 VSCode 扩展底层的 WebAssembly 模块）

## 项目结构与 Monorepo 划分

项目包含三个子 crate，职责必须严格隔离：

1.  **`texpand-core`**：核心逻辑。**约束：** 必须保持 I/O 无关，绝对禁止直接调用 `std::fs` 或 `std::io`。所有文件读取必须通过 `FileResolver` Trait 抽象。
2.  **`texpand-cli`**：CLI 前端。**职责：** 解析命令行参数 (clap)，实现 `FileResolver` 以读取本地磁盘文件，调用 `texpand-core` 并输出结果。
3.  **`texpand-vscode`**：VSCode 扩展前端。**职责：** 通过 TypeScript 调用 VSCode API 读取文件，将数据传递给通过 `wasm-bindgen` 编译的 Rust WASM 模块。

## 核心架构设计

### 1. I/O 抽象

为了实现跨端，核心库定义了如下数据获取接口：

```rust
// texpand-core/src/resolver.rs
pub trait FileResolver {
    // 传入 #include 后的相对路径，返回 (文件的绝对路径/标识符, 文件源码文本)
    fn resolve_and_read(&self, include_path: &str) -> Result<(String, String), String>;
}
```

- CLI 端：使用 `std::fs` 根据配置的 `include_paths` 搜索并读取。
- WASM 端：TypeScript 端封装 `vscode.workspace.fs.readFile`，Rust 端通过 `extern "C"` 导入该 JS 异步/同步函数来实现 Trait。

### 2. 依赖展开路径

1. 利用 Tree-sitter 解析文件，过滤提取出 `preproc_include` 节点。
2. 通过 `FileResolver` 获取文件内容。
3. 使用 `petgraph` 构建依赖有向图。图的节点为文件路径，边为包含关系。
4. 执行 Tarjan 算法检测是否存在环。若有环，提取环路径并抛出致命错误。
5. 按照后序遍历 (Post-order DFS) 的顺序自底向上拼接源码。

### 3. 代码压缩安全原则

代码压缩旨在减小体积，但**语义安全是最高优先级**。压缩逻辑基于 Tree-sitter 的 AST 叶子节点：

* **注释丢弃**：直接丢弃 `kind == "comment"` 的节点。
* **标识符隔离**：维护状态机。如果相邻的两个 Token，前一个的尾字符是 `[a-zA-Z0-9_]` 且后一个的首字符也是 `[a-zA-Z0-9_]`，则它们之间**必须强制插入一个空格**。
* **符号紧凑**：除上述情况外，纯符号（如 `{`, `+`, `;`）之间直接拼接，不加空格。

### 4. 关键边界 Case 处理 (预处理指令截断)

**注意：** C/C++ 预处理指令（`#define`, `#include` 等）对换行符敏感。基础压缩会抹除所有换行，这会导致预处理指令吞噬后续代码。在遍历 AST 时，如果游标进入任何 `preproc_*` 节点，必须在其遍历结束（离开该节点作用域）时，向输出缓冲区**强制追加一个换行符 `\n`**。
