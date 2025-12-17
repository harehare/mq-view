<h1 align="center">mq-view</h1>

[![ci](https://github.com/harehare/mq-view/actions/workflows/ci.yml/badge.svg)](https://github.com/harehare/mq-view/actions/workflows/ci.yml)

A library and CLI tool for rendering Markdown documents with syntax highlighting and rich text formatting.
Built with [mq](https://github.com/harehare/mq) - jq-like command-line tool for markdown processing.

![demo](assets/demo.gif)

## Features

- 🎨 **Syntax Highlighting**: Tree-sitter powered syntax highlighting for 13+ programming languages
- 📝 **Rich Markdown Rendering**: Support for headers, lists, code blocks, links, images, and more
- 🔔 **GitHub-style Callouts**: NOTE, TIP, IMPORTANT, WARNING, CAUTION
- 🔗 **Clickable Links**: Terminal hyperlinks using OSC 8

## Installation

### Quick Install

```bash
curl -sSL https://raw.githubusercontent.com/harehare/mq-view/refs/heads/main/bin/install.sh | bash
```

The installer will:
- Download the latest mq-view binary for your platform
- Install it to `~/.mq/bin/`
- Update your shell profile to add mq-view to your PATH

### Cargo

From crates.io (stable):

```sh
cargo install mq-view
```

From git (latest):

```sh
cargo install --git https://github.com/harehare/mq-view.git
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

## License

MIT
