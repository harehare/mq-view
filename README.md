<div align="center">
  <img src="assets/logo.svg" style="width: 128px; height: 128px;"/>

<h1>mq-view</h1>

[![ci](https://github.com/harehare/mq-view/actions/workflows/ci.yml/badge.svg)](https://github.com/harehare/mq-view/actions/workflows/ci.yml)

</div>

A library and CLI tool for rendering Markdown documents with syntax highlighting and rich text formatting.
Built with [mq](https://github.com/harehare/mq) - jq-like command-line tool for markdown processing.

![demo](assets/demo.gif)

## Features

- 🎨 **Syntax Highlighting**: Tree-sitter powered syntax highlighting for 29+ programming and config languages
- 📝 **Rich Markdown Rendering**: Support for headers, lists, code blocks, links, images, tables, and more
- 🧜 **Mermaid Diagrams**: Best-effort ASCII-art rendering of simple `graph`/`flowchart` blocks
- 🔔 **GitHub-style Callouts**: NOTE, TIP, IMPORTANT, WARNING, CAUTION, rendered as wrapped, bordered boxes
- 🔗 **Clickable Links**: Terminal hyperlinks using OSC 8
- 📖 **Pager Mode**: Interactive full-screen viewer with scrolling, a heading outline, link navigation with back/forward history, mouse support, and auto-reload on file changes
- 🔎 **mq Query Filtering**: Filter the document through an [mq](https://github.com/harehare/mq) query before rendering
- 🎨 **Themes**: Dark/light color palettes (with auto-detection) and `NO_COLOR` support
- 🔢 **Line Numbers**: Optional line-number gutter on code blocks

## Installation

### Quick Install

```bash
curl -sSL https://raw.githubusercontent.com/harehare/mq-view/refs/heads/main/bin/install.sh | bash
```

The installer will:
- Download the latest mq-view binary for your platform
- Install it to `~/.local/bin/`
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

Enabled by default:

- Rust, JavaScript, TypeScript (+ TSX), Python
- HTML, CSS, JSON, YAML, TOML
- Bash/Shell, Ruby, SQL
- Elixir, mq

Available with the `all-languages` feature:

- Go, Java, Kotlin, Scala
- C, C++, Swift
- PHP, Lua, Clojure, Haskell, OCaml, Elm
- Dockerfile, Makefile

See `Cargo.toml` for the full list of `lang-*` feature flags if you only need
one or two extra languages instead of all of them.

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

### Themes and Colors

```bash
mq-view --theme dark README.md    # force the dark palette
mq-view --theme light README.md   # force the light palette
mq-view --theme auto README.md    # default: guess from the COLORFGBG env var, fall back to dark
mq-view --no-color README.md      # disable all ANSI color output (also respects $NO_COLOR)
```

### Line Numbers

```bash
mq-view --line-numbers README.md  # or -n
```

In `--pager` mode, `L` toggles the line-number gutter at runtime.

### Pager Mode

Open an interactive, full-screen viewer with `--pager` (`-p`):

```bash
mq-view --pager README.md
```

It also works with piped content, but without a file to watch there's
nothing to auto-reload:

```bash
cat report.md | mq-view --pager
```

| Key | Action |
| --- | --- |
| `j` / `k`, `↓` / `↑` | Scroll down / up |
| `Space` / `PageDown` / `f`, `PageUp` / `b` | Scroll a page down / up |
| `d` / `u` (with or without `Ctrl`) | Scroll half a page down / up |
| `g` / `Home`, `G` / `End` | Jump to top / bottom |
| `Tab` | Toggle the heading outline; `j`/`k` to move, `Enter` to jump |
| `Enter` | Open the link list; `j`/`k` to move, `Enter` to follow, `Esc` to cancel |
| `[` / `]` | Go back / forward through followed links |
| `L` | Toggle the code-block line-number gutter |
| `/` | Search; `Enter` to confirm, `Esc` to cancel |
| `n` / `N` | Jump to the next / previous search match |
| Mouse wheel | Scroll (or move the selection inside an open list) |
| Mouse click | Select and jump to an item in the heading/link list |
| `q` / `Esc` | Quit |

When viewing a file (not piped input), the pager watches it and
automatically re-renders whenever it changes on disk. Following a link to
another local Markdown file re-points the watcher at that file; the
`--query` filter (if any) only applies to the file mq-view was originally
opened with, not to files reached by following a link.

Links are resolved as: `#anchor` jumps to a matching heading in the current
document; `scheme://...` and `mailto:` links open in your OS's default
handler; anything else is treated as a path relative to the current file.

### mq Query Filtering

Pass `-q`/`--query` with an [mq](https://github.com/harehare/mq) query to filter the
document before rendering — works with both plain output and `--pager`:

```bash
mq-view --query '.h' README.md          # only headings
mq-view --query '.code | select(.lang == "rust")' README.md
mq-view --pager --query '.h' README.md
```

In `--pager` mode, the query is re-applied on every auto-reload as well.

### Mermaid Diagrams

Fenced code blocks tagged ` ```mermaid ` are rendered as ASCII art instead of
plain text when the diagram is a simple `graph`/`flowchart`:

```mermaid
graph TD
    A[Start] --> B{Is it working?}
    B -->|Yes| C[Great success]
    B -->|No| D[Debug it]
```

This only understands a small subset of mermaid flowchart syntax (nodes,
shapes, and edges with optional labels). Other diagram types (sequence,
class, gantt, ...) and advanced flowchart syntax fall back to a regular,
syntax-highlighted code block.

## License

MIT
