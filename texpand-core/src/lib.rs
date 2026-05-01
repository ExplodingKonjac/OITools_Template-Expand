//! texpand-core: core template expansion library.
//! Kept I/O-free; all file reading must go through the `FileResolver` trait.

pub mod compressor;
pub mod expander;
pub mod graph;
pub mod parser;
pub mod resolver;
