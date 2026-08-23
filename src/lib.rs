//! A library for rendering Markdown documents with syntax highlighting.
//!
//! This crate provides functionality to render Markdown content with rich text formatting
//! and syntax highlighting for code blocks using tree-sitter.
//!
//! # Examples
//!
//! ```rust
//! use mq_view::render_markdown_to_string;
//! use mq_markdown::Markdown;
//!
//! let markdown: Markdown = "# Hello\n\n```rust\nfn main() {}\n```".parse().unwrap();
//! let rendered = render_markdown_to_string(&markdown).unwrap();
//! println!("{}", rendered);
//! ```

mod highlighter;
mod mermaid;
mod pager;
mod renderer;
mod theme;

pub use highlighter::SyntaxHighlighter;
pub use pager::run_pager;
pub use renderer::{
    RenderConfig, render_markdown, render_markdown_to_string, render_markdown_with_config,
};
pub use theme::{Theme, ThemeMode};

/// Runs an mq query against `content` and re-serializes the resulting nodes
/// back to Markdown source, so the result can be fed into the same parse
/// path as any other Markdown document.
pub fn apply_query(content: &str, query: &str) -> miette::Result<String> {
    let markdown: mq_markdown::Markdown = content
        .parse()
        .map_err(|e| miette::miette!("Markdown parse error: {}", e))?;
    let mut engine: mq_lang::Engine = mq_lang::Engine::default();
    engine.load_builtin_module();
    let inputs = markdown.nodes.into_iter().map(mq_lang::RuntimeValue::from);
    let results = engine
        .eval(query, inputs)
        .map_err(|e| miette::miette!("Query error: {}", e))?;
    let nodes = results
        .into_iter()
        .map(|value| match value {
            mq_lang::RuntimeValue::Markdown(node, _) => *node,
            other => other.to_string().into(),
        })
        .collect();
    Ok(mq_markdown::Markdown::new(nodes).to_string())
}
