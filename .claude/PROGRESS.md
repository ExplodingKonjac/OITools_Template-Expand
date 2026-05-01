# texpand 项目进度

## 总体状态

| Phase | 描述 | 状态 |
|-------|------|------|
| 1 | Workspace 骨架搭建 | ✅ 完成 |
| 2 | texpand-core 核心逻辑 | ✅ 完成 |
| 3 | texpand-cli CLI 前端 | ✅ 完成 |
| 4 | texpand-vscode VSCode 扩展 | 🏗️ 进行中 |
| 5 | 边缘 Case 加固与文档 | ⏳ 待开始 |

## Phase 1: Workspace 骨架搭建 ✅

- [x] 根 `Cargo.toml` → workspace 定义（3 成员）
- [x] `texpand-core/` lib crate 创建
- [x] `texpand-cli/` bin crate 创建
- [x] `texpand-vscode/` cdylib crate 创建
- [x] 依赖添加：tree-sitter, tree-sitter-cpp, petgraph, clap, serde, toml, wasm-bindgen, anyhow
- [x] 旧 `src/main.rs` 移除
- [x] `cargo check --workspace` ✅

## Phase 2: texpand-core 核心逻辑 ✅

### resolver.rs
- [x] `FileResolver` trait 定义

### parser.rs
- [x] `parse_source()` — tree-sitter 封装
- [x] `extract_all_includes()` — 提取 Local / System include
- [x] `extract_include_paths()` — 仅 Local include（BFS 用）
- [x] `is_quoted_include()` — 判断是否为本地 include

### graph.rs
- [x] `DependencyGraph` — petgraph 有向图
- [x] `add_file()` / `add_dependency()` — 节点/边管理
- [x] `detect_cycle()` — Tarjan SCC 环路检测
- [x] `expansion_order()` — 逆拓扑序输出

### expander.rs
- [x] `expand()` — BFS 全量展开
- [x] Local include → strip + 递归解析
- [x] System include → 保留原行 + 入图
- [x] 可选 compression 分支

### compressor.rs
- [x] 注释丢弃
- [x] 标识符隔离（`[a-zA-Z0-9_]` 相邻时插空格）
- [x] 符号紧凑
- [x] preproc 节点换行保护
- [x] let-chains 风格（Rust edition 2024）

### 测试覆盖
- [x] 25 个单元测试全部通过
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo fmt --all` 通过

## Phase 3: texpand-cli ✅

- [x] CLI args（clap）：`INPUT`, `-c`, `--no-compress`, `-i`, `-o`, `-C`, `--config`
- [x] `config.rs`：`include_paths`, `default_compress` TOML 解析
- [x] `FsResolver`：`FileResolver` 的 `std::fs` 实现（支持 includer 相对路径解析）
- [x] pipeline 组装
- [x] 修复 expander 核心 bug：图节点键名需使用 canonicalized 路径而非原始 include 路径
- [x] 改进 `FileResolver` trait：新增 `includer_path` 参数，支持嵌套目录相对 `#include` 解析
- [x] 添加测试用 fixtures（`fixtures/`）：basic, deep, diamond, system-include, pragma-once, compress, multiple, nested, circular

## Phase 4: texpand-vscode 🏗️

- [x] VSCode 扩展脚手架（yo code）
- [x] WASM 编译（wasm32-wasip1, WASI SDK）
- [x] `WasiFsResolver` — 利用 WASI 文件系统的 `std::fs` 实现
- [x] 3 个注册命令（expandDefault, expandAndCopy, expandToNewFile）
- [x] 编辑器标题栏按钮（`$(copy)` 图标，c/c++ 专用）
- [x] 右键上下文菜单（Texpand 子菜单，含两个选项）
- [x] 底部状态栏（配置 QuickPick：压缩、输出模式、搜索路径）
- [ ] 端到端集成调试（需要实际 VSCode 实例加载扩展）
- [ ] 扩展打包与发布（vsce package）

## Phase 5: 边缘 Case 加固 ⏳

- [ ] 极端 C/C++ 语法测试样例
- [ ] 补充单元测试
- [ ] 各 crate API 文档

## 已知技术债务

_（所有已知债务已解决）_
