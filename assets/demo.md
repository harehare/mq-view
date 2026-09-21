# mq Demo

## Introduction

`mq` is a jq-like command-line tool for Markdown processing.

> [!TIP]
> Pipe any Markdown file into `mq-view` to get syntax highlighting,
> callouts, tables, and mermaid diagrams right in your terminal.

## Links

Links are clickable in your terminal, and in pager mode you can follow them
without leaving the viewer:

- Web: [mq on GitHub](https://github.com/harehare/mq) and the [project site](https://mqlang.org)
- Same document: [Jump to the table](#tables)
- Another file: [More examples](demo-links.md)

## Code Examples

```rust
fn main() {
    println!("Hello, mq!");
}
```

## How It Works

```mermaid
graph LR
    A[Markdown] --> B[mq]
    B --> C[mq-view]
```

## Tables

| Feature     | Description              | Status |
| ----------- | ------------------------ | ------ |
| Headers     | Filter headers by level  | ✅      |
| Callouts    | NOTE, TIP, WARNING, ...  | ✅      |
| Tables      | Process markdown tables  | ✅      |
| Mermaid     | Render simple flowcharts | ✅      |

> [!WARNING]
> Always double-check generated queries before running them on real data.

Try mq today!
