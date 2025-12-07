<h1 align="center">mq-view</h1>

[![ci](https://github.com/harehare/mq-view/actions/workflows/ci.yml/badge.svg)](https://github.com/harehare/mq-view/actions/workflows/ci.yml)
[![mq language](https://img.shields.io/badge/mq-language-orange.svg)](https://github.com/harehare/mq)

A library and CLI tool for rendering Markdown documents with syntax highlighting and rich text formatting.
Built with [mq](https://github.com/harehare/mq) - jq-like command-line tool for markdown processing.

![demo](assets/demo.gif)

## Features

- 🎨 **Syntax Highlighting**: Tree-sitter powered syntax highlighting for 13+ programming languages
- 📝 **Rich Markdown Rendering**: Support for headers, lists, code blocks, links, images, and more
- 🔔 **GitHub-style Callouts**: NOTE, TIP, IMPORTANT, WARNING, CAUTION
- 🔗 **Clickable Links**: Terminal hyperlinks using OSC 8
- 📦 **Library and CLI**: Use as a library or standalone CLI tool

## Installation

### Quick Install

```bash
curl -sSL https://raw.githubusercontent.com/harehare/mq-view/refs/heads/main/bin/install.sh | bash
```

The installer will:
- Download the latest mq-view binary for your platform
- Install it to `~/.mq-view/bin/`
- Update your shell profile to add mq-view to your PATH

### Cargo

```sh
$ cargo install --git https://github.com/harehare/mq-view.git
$ cargo install mq-view
```

## Supported Languages

- Rust, JavaScript, TypeScript (+ TSX)
- Python, Go, Java
- C, C++
- HTML, CSS, JSON
- Bash/Shell

## Usage

### As a CLI Tool

View a markdown file:

```bash
mq-view README.md
```

Pipe markdown content:

```bash
echo "# Hello\n\n\`\`\`rust\nfn main() {}\n\`\`\`" | mq-view
```

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
mq-view = {git = "https://github.com/harehare/mq-view.git"}
```

Use in your code:

```rust
use mq-view::{render_markdown, render_markdown_to_string};
use mq_markdown::Markdown;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let markdown: Markdown = "# Hello\n\nWorld".parse()?;

    // Render to a writer
    let mut output = Vec::new();
    render_markdown(&markdown, &mut output)?;

    // Or render to a string
    let rendered = render_markdown_to_string(&markdown)?;
    println!("{}", rendered);

    Ok(())
}
```

## API Documentation

### `render_markdown`

Render a Markdown document to any writer that implements `Write`:

```rust
pub fn render_markdown<W: Write>(markdown: &Markdown, writer: &mut W) -> io::Result<()>
```

### `render_markdown_to_string`

Render a Markdown document to a `String`:

```rust
pub fn render_markdown_to_string(markdown: &Markdown) -> io::Result<String>
```

### `SyntaxHighlighter`

Create and use a syntax highlighter independently:

```rust
use mq-view::SyntaxHighlighter;

let mut highlighter = SyntaxHighlighter::new();
let code = "fn main() { println!(\"Hello\"); }";
let highlighted = highlighter.highlight(code, Some("rust"));
println!("{}", highlighted);
```

## Examples

See the [examples](examples/) directory for more usage examples.

Run an example:

```bash
cargo run --example basic_usage
```

## License

MIT
