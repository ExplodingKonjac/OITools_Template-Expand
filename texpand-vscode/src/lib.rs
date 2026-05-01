//! texpand-vscode: VSCode extension WASM frontend.
//! Exposes expansion functions via wasm-bindgen.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn expand(source: &str) -> String {
    // TODO: will call into texpand-core
    format!("expanded: {}", source)
}
