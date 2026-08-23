use crate::highlighter::SyntaxHighlighter;
use colored::*;
use mq_markdown::{Markdown, Node};
use std::io::{self, Write};
use std::path::Path;
use std::sync::LazyLock;
use terminal_size::{Height, Width, terminal_size};
use unicode_width::UnicodeWidthStr;

/// Configuration for rendering markdown
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Enable full-width background highlighting for headers
    pub header_full_width_highlight: bool,
    /// Draw local images directly to the terminal via `viuer`. This writes
    /// straight to the real stdout, bypassing the writer passed to the
    /// render functions, so it must be disabled by callers (like the pager)
    /// that manage their own alternate-screen terminal buffer.
    pub inline_images: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            header_full_width_highlight: true,
            inline_images: true,
        }
    }
}

/// Unicode bullet symbols for lists
const LIST_BULLETS: &[&str] = &["●", "○", "◆", "◇"];

static WIDTH: LazyLock<usize> = LazyLock::new(|| {
    let size = terminal_size();
    if let Some((Width(w), Height(_))) = size {
        w.into()
    } else {
        80
    }
});

/// GitHub-style callout definitions
#[derive(Debug, Clone)]
struct Callout {
    icon: &'static str,
    color: colored::Color,
    name: &'static str,
}

const CALLOUTS: &[(&str, Callout)] = &[
    (
        "NOTE",
        Callout {
            icon: "ℹ️",
            color: colored::Color::Blue,
            name: "Note",
        },
    ),
    (
        "TIP",
        Callout {
            icon: "💡",
            color: colored::Color::Green,
            name: "Tip",
        },
    ),
    (
        "IMPORTANT",
        Callout {
            icon: "❗",
            color: colored::Color::Magenta,
            name: "Important",
        },
    ),
    (
        "WARNING",
        Callout {
            icon: "⚠️",
            color: colored::Color::Yellow,
            name: "Warning",
        },
    ),
    (
        "CAUTION",
        Callout {
            icon: "🔥",
            color: colored::Color::Red,
            name: "Caution",
        },
    ),
];

/// Create a clickable link using ANSI escape sequences (OSC 8)
/// Format: ESC ] 8 ; params ; URI ST display_text ESC ] 8 ; ; ST
fn make_clickable_link(url: &str, display_text: &str) -> String {
    // Using ST (String Terminator) \x1b\\ instead of BEL \x07 for better compatibility
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, display_text)
}

/// Render a Markdown document to a writer with syntax highlighting and rich text formatting.
///
/// # Errors
///
/// Returns an `io::Error` if writing to the output fails.
///
/// # Examples
///
/// ```rust
/// use mq_view::render_markdown;
/// use mq_markdown::Markdown;
/// use std::io::BufWriter;
///
/// let markdown: Markdown = "# Hello\n\nWorld".parse().unwrap();
/// let mut output = Vec::new();
/// {
///     let mut writer = BufWriter::new(&mut output);
///     render_markdown(&markdown, &mut writer).unwrap();
/// }
/// ```
pub fn render_markdown<W: Write>(markdown: &Markdown, writer: &mut W) -> io::Result<()> {
    render_markdown_with_config(markdown, writer, &RenderConfig::default())
}

/// Render a Markdown document to a writer with custom configuration.
///
/// # Errors
///
/// Returns an `io::Error` if writing to the output fails.
pub fn render_markdown_with_config<W: Write>(
    markdown: &Markdown,
    writer: &mut W,
    config: &RenderConfig,
) -> io::Result<()> {
    let mut highlighter = SyntaxHighlighter::new();
    let mut i = 0;
    let len = markdown.nodes.len();

    while i < len {
        let node = &markdown.nodes[i];
        if matches!(node, Node::TableCell(_)) {
            // Collect consecutive table-related nodes
            let table_nodes: Vec<&Node> = markdown.nodes[i..]
                .iter()
                .take_while(|n| {
                    matches!(
                        n,
                        Node::TableCell(_) | Node::TableAlign(_) | Node::TableRow(_)
                    )
                })
                .collect();
            render_table(&table_nodes, &mut highlighter, writer)?;
            i += table_nodes.len();
        } else {
            render_node(node, 0, &mut highlighter, config, writer)?;
            i += 1;
        }
    }
    Ok(())
}

/// Render a Markdown document to a String with syntax highlighting and rich text formatting.
///
/// # Examples
///
/// ```rust
/// use mq_view::render_markdown_to_string;
/// use mq_markdown::Markdown;
///
/// let markdown: Markdown = "# Hello\n\nWorld".parse().unwrap();
/// let rendered = render_markdown_to_string(&markdown).unwrap();
/// println!("{}", rendered);
/// ```
pub fn render_markdown_to_string(markdown: &Markdown) -> io::Result<String> {
    let mut output = Vec::new();
    render_markdown(markdown, &mut output)?;
    String::from_utf8(output).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// A heading found while rendering, with the rendered (post-wrap) line
/// number it starts on so a viewer can jump straight to it.
#[derive(Debug, Clone)]
pub(crate) struct HeadingEntry {
    pub line: usize,
    pub depth: u8,
    pub title: String,
}

/// Same as [`render_markdown_with_config`], but also returns the outline of
/// headings alongside the rendered line each one lands on. Used by the
/// pager to build a jump-to-heading sidebar.
pub(crate) fn render_markdown_with_outline(
    markdown: &Markdown,
    config: &RenderConfig,
) -> io::Result<(String, Vec<HeadingEntry>)> {
    let mut highlighter = SyntaxHighlighter::new();
    let mut output: Vec<u8> = Vec::new();
    let mut headings = Vec::new();
    let mut i = 0;
    let len = markdown.nodes.len();

    while i < len {
        let node = &markdown.nodes[i];
        if let Node::Heading(heading) = node {
            // Headings always start with a blank separator line (see
            // render_node_inline), so the title itself lands one line later.
            let line = bytecount_newlines(&output) + 1;
            headings.push(HeadingEntry {
                line,
                depth: heading.depth,
                title: render_inline_content(&heading.values),
            });
        }

        if matches!(node, Node::TableCell(_)) {
            let table_nodes: Vec<&Node> = markdown.nodes[i..]
                .iter()
                .take_while(|n| {
                    matches!(
                        n,
                        Node::TableCell(_) | Node::TableAlign(_) | Node::TableRow(_)
                    )
                })
                .collect();
            render_table(&table_nodes, &mut highlighter, &mut output)?;
            i += table_nodes.len();
        } else {
            render_node(node, 0, &mut highlighter, config, &mut output)?;
            i += 1;
        }
    }

    let rendered =
        String::from_utf8(output).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok((rendered, headings))
}

fn bytecount_newlines(buf: &[u8]) -> usize {
    buf.iter().filter(|&&b| b == b'\n').count()
}

/// Visible column width of a string, ignoring ANSI escape sequences
/// (SGR color codes and OSC 8 hyperlinks) so that box borders and wrapping
/// stay aligned even when the content contains colored or clickable text.
/// Visible runs are measured with their real terminal column width (not a
/// raw `char` count), so wide CJK text and emoji - including multi-codepoint
/// sequences like an emoji + variation selector - line up correctly too.
pub(crate) fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut run = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            width += UnicodeWidthStr::width(run.as_str());
            run.clear();
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for c2 in chars.by_ref() {
                        if c2.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(c2) = chars.next() {
                        if c2 == '\x07' {
                            break;
                        }
                        if c2 == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        run.push(c);
    }
    width += UnicodeWidthStr::width(run.as_str());
    width
}

/// Greedily word-wrap `s` so each line's visible width fits within `width`
/// columns (ANSI escapes don't count toward the width).
fn wrap_visible(s: &str, width: usize) -> Vec<String> {
    if width == 0 || s.trim().is_empty() {
        return vec![s.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for word in s.split(' ').filter(|w| !w.is_empty()) {
        let word_width = visible_width(word);
        if current.is_empty() {
            current = word.to_string();
            current_width = word_width;
        } else if current_width + 1 + word_width <= width {
            current.push(' ');
            current.push_str(word);
            current_width += 1 + word_width;
        } else {
            lines.push(current);
            current = word.to_string();
            current_width = word_width;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Render `lines` inside a bordered box. Lines are never wrapped (so
/// syntax-highlighted code keeps its original structure); a line wider than
/// the box just overflows past the right border instead of being cut.
fn box_inner_width(header_width: usize) -> usize {
    WIDTH.saturating_sub(4).max(header_width + 2)
}

fn render_boxed_lines<W: Write>(
    writer: &mut W,
    header: Option<&str>,
    color: colored::Color,
    lines: &[String],
) -> io::Result<()> {
    let header_width = header.map(visible_width).unwrap_or(0);
    let inner_width = box_inner_width(header_width);
    let border = "─".repeat(inner_width + 2);

    let top = match header {
        Some(h) if !h.is_empty() => format!(
            "┌─ {} {}┐",
            h,
            "─".repeat(inner_width.saturating_sub(header_width + 1))
        ),
        _ => format!("┌{}┐", border),
    };
    writeln!(writer, "{}", top.color(color))?;

    for line in lines {
        let w = visible_width(line);
        if w <= inner_width {
            let pad = inner_width - w;
            writeln!(
                writer,
                "{} {}{} {}",
                "│".color(color),
                line,
                " ".repeat(pad),
                "│".color(color)
            )?;
        } else {
            writeln!(writer, "{} {}", "│".color(color), line)?;
        }
    }

    writeln!(writer, "{}", format!("└{}┘", border).color(color))?;
    Ok(())
}

fn detect_callout(text: &str) -> Option<&'static Callout> {
    let trimmed = text.trim();
    if trimmed.starts_with("[!")
        && trimmed.contains(']')
        && let Some(end) = trimmed.find(']')
    {
        let callout_type = &trimmed[2..end];
        return CALLOUTS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(callout_type))
            .map(|(_, callout)| callout);
    }
    None
}

fn render_node<W: Write>(
    node: &Node,
    depth: usize,
    highlighter: &mut SyntaxHighlighter,
    config: &RenderConfig,
    writer: &mut W,
) -> io::Result<()> {
    render_node_inline(node, depth, false, highlighter, config, writer)
}

fn render_node_inline<W: Write>(
    node: &Node,
    depth: usize,
    inline: bool,
    highlighter: &mut SyntaxHighlighter,
    config: &RenderConfig,
    writer: &mut W,
) -> io::Result<()> {
    match node {
        Node::Heading(heading) => {
            if !inline {
                writeln!(writer)?;
            }

            // Repeat the marker once per heading level (▶, ▶▶, ▶▶▶, ...) so
            // depth stays legible even in fonts that render circled digits
            // (①②③) too small to read at a glance.
            let symbol = "▶".repeat(heading.depth.clamp(1, 6) as usize);

            let text = render_inline_content(&heading.values);

            if config.header_full_width_highlight {
                let padding =
                    WIDTH.saturating_sub(visible_width(&text) + visible_width(&symbol) + 2);
                let line = format!("{}{}", text, " ".repeat(padding));

                // Full-width background highlighting
                match heading.depth {
                    1 => {
                        writeln!(
                            writer,
                            "{}{}{}",
                            symbol.bold().black().on_bright_blue(),
                            "  ".on_bright_blue(),
                            line.bold().bright_black().on_bright_blue()
                        )?;
                    }
                    2 => {
                        writeln!(
                            writer,
                            "{}{}{}",
                            symbol.bold().black().on_cyan(),
                            "  ".on_cyan(),
                            line.bold().bright_black().on_cyan()
                        )?;
                    }
                    3 => {
                        writeln!(
                            writer,
                            "{}{}{}",
                            symbol.bold().black().on_yellow(),
                            "  ".on_yellow(),
                            line.bold().bright_black().on_yellow()
                        )?;
                    }
                    4 => {
                        writeln!(
                            writer,
                            "{}{}{}",
                            symbol.bold().black().on_green(),
                            "  ".on_green(),
                            line.bold().bright_black().on_green()
                        )?;
                    }
                    5 => {
                        writeln!(
                            writer,
                            "{}{}{}",
                            symbol.bold().black().on_magenta(),
                            "  ".on_magenta(),
                            line.bold().bright_black().on_magenta()
                        )?;
                    }
                    _ => {
                        writeln!(writer, "{}  {}", symbol.bold().white(), text.bold().white())?;
                    }
                }
            } else {
                // Simple header without full-width highlighting
                match heading.depth {
                    1 => {
                        writeln!(
                            writer,
                            "{}  {}",
                            symbol.bold().bright_blue(),
                            text.bold().bright_blue()
                        )?;
                    }
                    2 => {
                        writeln!(writer, "{}  {}", symbol.bold().cyan(), text.bold().cyan())?;
                    }
                    3 => {
                        writeln!(
                            writer,
                            "{}  {}",
                            symbol.bold().yellow(),
                            text.bold().yellow()
                        )?;
                    }
                    4 => {
                        writeln!(writer, "{}  {}", symbol.bold().green(), text.bold().green())?;
                    }
                    5 => {
                        writeln!(
                            writer,
                            "{}  {}",
                            symbol.bold().magenta(),
                            text.bold().magenta()
                        )?;
                    }
                    _ => {
                        writeln!(writer, "{}  {}", symbol.bold().white(), text.bold().white())?;
                    }
                }
            }
            writeln!(writer)?;
        }

        Node::Text(text) => {
            if !text.value.trim().is_empty() {
                if inline {
                    write!(writer, "{}", text.value)?;
                } else {
                    writeln!(writer, "{}", text.value)?;
                }
            }
        }

        Node::List(list) => {
            render_list(list, depth, highlighter, config, writer)?;
        }

        Node::Code(code) => {
            let is_mermaid = code
                .lang
                .as_deref()
                .is_some_and(|lang| lang.eq_ignore_ascii_case("mermaid"));

            let mermaid_diagram = is_mermaid
                .then(|| crate::mermaid::render(&code.value, *WIDTH))
                .flatten();

            if let Some(diagram) = mermaid_diagram {
                writeln!(writer)?;
                write!(writer, "{}", diagram)?;
                writeln!(writer)?;
            } else {
                // Apply syntax highlighting if language is specified
                let highlighted = highlighter.highlight(&code.value, code.lang.as_deref());
                let lines: Vec<String> = highlighted
                    .strip_suffix('\n')
                    .unwrap_or(&highlighted)
                    .split('\n')
                    .map(str::to_string)
                    .collect();

                writeln!(writer)?;
                render_boxed_lines(
                    writer,
                    code.lang.as_deref(),
                    colored::Color::BrightBlack,
                    &lines,
                )?;
                writeln!(writer)?;
            }
        }

        Node::CodeInline(code) => {
            write!(writer, "{}", format!("`{}`", code.value).bright_yellow())?;
        }

        Node::Strong(strong) => {
            write!(writer, "{}", render_inline_content(&strong.values).bold())?;
        }

        Node::Emphasis(emphasis) => {
            write!(
                writer,
                "{}",
                render_inline_content(&emphasis.values).italic()
            )?;
        }

        Node::Link(link) => {
            let text = render_inline_content(&link.values);
            let url = link.url.as_str();

            if text.trim().is_empty() {
                // If no link text, just make the URL clickable
                write!(
                    writer,
                    " {} {}",
                    "🔗".bright_blue(),
                    make_clickable_link(url, url)
                )?;
            } else {
                // Make the title clickable without showing URL
                write!(
                    writer,
                    " {} {}",
                    "🔗".bright_blue(),
                    make_clickable_link(url, &text).underline().bright_blue()
                )?;
            }
        }

        Node::Image(image) => {
            let alt = image.alt.as_str();
            let url = image.url.as_str();

            if config.inline_images {
                let _ = render_image_to_terminal(url);
            }

            // Always show the text description as well
            if alt.trim().is_empty() {
                writeln!(
                    writer,
                    "{} {}",
                    "🖼️ ".bright_green(),
                    url.underline().bright_green()
                )?;
            } else {
                writeln!(
                    writer,
                    "{} {} ({})",
                    "🖼️ ".bright_green(),
                    alt.bright_green(),
                    url.bright_black()
                )?;
            }
        }

        Node::HorizontalRule(_) => {
            writeln!(writer, "{}", "─".repeat(80).bright_black())?;
            writeln!(writer)?;
        }

        Node::Blockquote(blockquote) => {
            if !inline {
                writeln!(writer)?;
            }

            // Check if this is a GitHub-style callout
            let is_callout = {
                let mut found_callout = false;
                // Check all nodes in blockquote for callout pattern
                for value in &blockquote.values {
                    match value {
                        Node::Fragment(para) => {
                            for child in &para.values {
                                if let Node::Text(text) = child
                                    && detect_callout(&text.value).is_some()
                                {
                                    found_callout = true;
                                    break;
                                }
                            }
                        }
                        Node::Text(text) if detect_callout(&text.value).is_some() => {
                            found_callout = true;
                            break;
                        }
                        _ => {}
                    }
                    if found_callout {
                        break;
                    }
                }
                found_callout
            };

            if is_callout {
                render_callout_blockquote(blockquote, writer)?;
            } else {
                render_regular_blockquote(blockquote, depth, highlighter, config, writer)?;
            }

            writeln!(writer)?;
        }

        // mq-markdown parses `> [!TYPE] ...` directly into this variant
        // rather than a plain Blockquote (see the `callout` feature in
        // Cargo.toml).
        Node::Callout(callout) => {
            if !inline {
                writeln!(writer)?;
            }
            render_native_callout(callout, writer)?;
            writeln!(writer)?;
        }

        Node::Html(html) => {
            // Apply syntax highlighting to HTML
            let highlighted = highlighter.highlight(&html.value, Some("html"));
            writeln!(writer, "{}", highlighted)?;
        }

        Node::Break(_) => {
            if inline {
                write!(writer, " ")?;
            } else {
                writeln!(writer)?;
            }
        }

        Node::Fragment(fragment) => {
            // Render paragraph as inline content on one line
            for child in &fragment.values {
                render_node_inline(child, depth, true, highlighter, config, writer)?;
            }
            // Add newline after paragraph unless we're inline
            if !inline {
                writeln!(writer)?;
            }
        }

        Node::TableAlign(_) | Node::TableRow(_) => {
            // These should be handled by render_table in render_markdown
            // If we encounter them here, skip them
        }

        Node::TableCell(cell) => {
            // Individual table cells outside of tables
            // Calculate column widths for this cell
            let column_widths = calculate_column_widths(&[Node::TableCell(cell.clone())]);
            render_table_cell(cell, &column_widths, highlighter, config, writer)?;
        }

        // Handle other node types recursively if they have children
        _ => {
            if let Some(children) = get_node_children(node) {
                for child in children {
                    render_node_inline(child, depth, inline, highlighter, config, writer)?;
                }
            }
        }
    }

    Ok(())
}

fn render_list<W: Write>(
    list: &mq_markdown::List,
    depth: usize,
    highlighter: &mut SyntaxHighlighter,
    config: &RenderConfig,
    writer: &mut W,
) -> io::Result<()> {
    let indent = "  ".repeat(depth);
    let bullet_index = depth % LIST_BULLETS.len();
    let bullet = if list.ordered {
        format!("{}.", list.index + 1)
    } else {
        LIST_BULLETS[bullet_index].to_string()
    };

    // Handle checkbox lists
    let checkbox = match list.checked {
        Some(true) => "☑️ ",
        Some(false) => "☐ ",
        None => "",
    };

    write!(writer, "{}{} {}", indent, bullet.bright_magenta(), checkbox)?;

    let mut has_content = false;
    for value in &list.values {
        match value {
            Node::List(nested_list) => {
                if has_content {
                    writeln!(writer)?; // New line before nested list only if we had content
                }
                render_list(nested_list, depth + 1, highlighter, config, writer)?;
            }
            Node::Fragment(fragment) => {
                // Handle paragraph content inline
                for child in &fragment.values {
                    render_node_inline(child, depth + 1, true, highlighter, config, writer)?;
                }
                has_content = true;
            }
            _ => {
                render_node_inline(value, depth + 1, true, highlighter, config, writer)?;
                has_content = true;
            }
        }
    }

    writeln!(writer)?; // Add line break after list item
    Ok(())
}

/// mq-markdown doesn't consistently wrap inline blockquote content in a
/// `Fragment`: a single-line callout comes through as flat `Text`/`Link`/...
/// nodes directly under `Blockquote`, while other inputs nest them inside a
/// `Fragment`. Flatten one level so callers can treat both shapes the same.
fn flatten_inline(values: &[Node]) -> Vec<&Node> {
    let mut out = Vec::new();
    for value in values {
        if let Node::Fragment(para) = value {
            out.extend(para.values.iter());
        } else {
            out.push(value);
        }
    }
    out
}

/// Render a single inline node's textual content for use inside a callout
/// box (where everything gets re-wrapped to the box width, so embedded
/// line breaks from the original markdown source are normalized to spaces).
fn inline_node_to_text(node: &Node) -> String {
    match node {
        Node::Text(text) => text.value.replace('\n', " "),
        Node::Link(link) => {
            let text = render_inline_content(&link.values);
            let url = link.url.as_str();
            if text.trim().is_empty() {
                format!(" 🔗 {}", make_clickable_link(url, url))
            } else {
                format!(" 🔗 {}", make_clickable_link(url, &text))
            }
        }
        Node::Break(_) => "\n".to_string(),
        other => render_inline_content(std::slice::from_ref(other)),
    }
}

fn render_callout_blockquote<W: Write>(
    blockquote: &mq_markdown::Blockquote,
    writer: &mut W,
) -> io::Result<()> {
    let inline_nodes = flatten_inline(&blockquote.values);

    // Find the marker node and the callout type it declares.
    let marker_idx = inline_nodes
        .iter()
        .position(|n| matches!(n, Node::Text(t) if detect_callout(&t.value).is_some()));
    let Some(marker_idx) = marker_idx else {
        return Ok(());
    };
    let Node::Text(marker_text) = inline_nodes[marker_idx] else {
        unreachable!()
    };
    let Some(callout) = detect_callout(&marker_text.value) else {
        unreachable!()
    };

    // Build one continuous string for the body: the part of the marker text
    // after `]`, followed by every later inline node's text. Soft line
    // breaks inside source text are normalized to spaces; explicit `Break`
    // nodes become paragraph separators (kept as `\n`) for re-wrapping.
    let mut body = String::new();
    if let Some(end) = marker_text.value.find(']') {
        body.push_str(&marker_text.value[end + 1..].replace('\n', " "));
    }
    for node in &inline_nodes[marker_idx + 1..] {
        body.push_str(&inline_node_to_text(node));
    }

    let mut content_lines: Vec<String> = Vec::new();
    for paragraph in body.split('\n') {
        if paragraph.trim().is_empty() {
            continue;
        }
        content_lines.push(paragraph.trim().to_string());
    }

    let header_text = format!("{} {}", callout.icon, callout.name);
    let inner_width = box_inner_width(visible_width(&header_text));
    let wrapped_lines: Vec<String> = content_lines
        .iter()
        .flat_map(|line| wrap_visible(line, inner_width))
        .collect();

    render_boxed_lines(writer, Some(&header_text), callout.color, &wrapped_lines)
}

fn render_native_callout<W: Write>(
    callout: &mq_markdown::Callout,
    writer: &mut W,
) -> io::Result<()> {
    let Some((_, def)) = CALLOUTS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(&callout.kind))
    else {
        return Ok(());
    };

    let mut content_lines: Vec<String> = Vec::new();
    if let Some(title) = &callout.title {
        content_lines.push(title.clone());
    }
    let body: String = flatten_inline(&callout.values)
        .into_iter()
        .map(inline_node_to_text)
        .collect();
    for paragraph in body.split('\n') {
        if paragraph.trim().is_empty() {
            continue;
        }
        content_lines.push(paragraph.trim().to_string());
    }

    let header_text = format!("{} {}", def.icon, def.name);
    let inner_width = box_inner_width(visible_width(&header_text));
    let wrapped_lines: Vec<String> = content_lines
        .iter()
        .flat_map(|line| wrap_visible(line, inner_width))
        .collect();

    render_boxed_lines(writer, Some(&header_text), def.color, &wrapped_lines)
}

fn render_regular_blockquote<W: Write>(
    blockquote: &mq_markdown::Blockquote,
    depth: usize,
    highlighter: &mut SyntaxHighlighter,
    config: &RenderConfig,
    writer: &mut W,
) -> io::Result<()> {
    for value in &blockquote.values {
        write!(writer, "{} ", "▌".bright_black())?;
        render_node_inline(value, depth, false, highlighter, config, writer)?;
    }
    Ok(())
}

fn render_inline_content(nodes: &[Node]) -> String {
    let mut result = String::new();
    for (i, node) in nodes.iter().enumerate() {
        // Add space between inline elements if needed
        if i > 0 && needs_space_before(node) && !result.ends_with(' ') {
            result.push(' ');
        }

        match node {
            Node::Text(text) => result.push_str(&text.value),
            Node::CodeInline(code) => result.push_str(&format!("`{}`", code.value)),
            Node::Strong(strong) => result.push_str(&render_inline_content(&strong.values)),
            Node::Emphasis(emphasis) => result.push_str(&render_inline_content(&emphasis.values)),
            Node::Link(link) => {
                let text = render_inline_content(&link.values);
                let url = link.url.as_str();
                if text.trim().is_empty() {
                    result.push_str(&format!("🔗 {}", make_clickable_link(url, url)));
                } else {
                    result.push_str(&format!("🔗 {}", make_clickable_link(url, &text)));
                }
            }
            _ => {}
        }
    }
    result
}

fn needs_space_before(node: &Node) -> bool {
    matches!(
        node,
        Node::Link(_) | Node::Strong(_) | Node::Emphasis(_) | Node::CodeInline(_)
    )
}

fn get_node_children(node: &Node) -> Option<&Vec<Node>> {
    match node {
        Node::Fragment(fragment) => Some(&fragment.values),
        Node::TableRow(row) => Some(&row.values),
        Node::TableCell(cell) => Some(&cell.values),
        _ => None,
    }
}

/// Render a complete table with proper column alignment
fn render_table<W: Write>(
    table_nodes: &[&Node],
    highlighter: &mut SyntaxHighlighter,
    writer: &mut W,
) -> io::Result<()> {
    if table_nodes.is_empty() {
        return Ok(());
    }

    // Tables don't use full-width highlighting, use default config
    let config = RenderConfig::default();

    // Calculate column widths from all cells
    let all_nodes: Vec<Node> = table_nodes.iter().map(|n| (*n).clone()).collect();
    let column_widths = calculate_column_widths(&all_nodes);

    // Find table header to determine column count
    let col_count = table_nodes
        .iter()
        .find_map(|node| {
            if let Node::TableAlign(header) = node {
                Some(header.align.len())
            } else {
                None
            }
        })
        .unwrap_or(column_widths.len());

    writeln!(writer)?;

    // Render top border
    render_table_top_border(&column_widths, col_count, writer)?;

    // Render cells row by row
    write!(writer, "{}", "│ ".bright_cyan())?;

    for (i, node) in table_nodes.iter().enumerate() {
        match node {
            Node::TableCell(cell) => {
                let content = render_inline_content(&cell.values);
                let width = column_widths.get(cell.column).copied().unwrap_or(0);

                for value in &cell.values {
                    render_node_inline(value, 0, true, highlighter, &config, writer)?;
                }

                // Pad with spaces to align columns
                let content_width = visible_width(&content);
                if content_width < width {
                    write!(writer, "{}", " ".repeat(width - content_width))?;
                }

                write!(writer, " {}", "│ ".bright_cyan())?;

                // Check if this is the last cell in its row
                let is_last_in_row = match table_nodes.get(i + 1) {
                    Some(Node::TableCell(next_cell)) => next_cell.row != cell.row,
                    _ => true,
                };

                if is_last_in_row {
                    writeln!(writer)?;
                    // Check if next node is the header separator or another cell
                    if i + 1 < table_nodes.len() {
                        if let Some(Node::TableAlign(header)) = table_nodes.get(i + 1) {
                            render_table_header(header, &column_widths, writer)?;
                            // After header, if there's another cell, start a new row
                            if i + 2 < table_nodes.len()
                                && matches!(table_nodes.get(i + 2), Some(Node::TableCell(_)))
                            {
                                write!(writer, "{}", "│ ".bright_cyan())?;
                            }
                        } else if matches!(table_nodes.get(i + 1), Some(Node::TableCell(_))) {
                            // Start new row
                            write!(writer, "{}", "│ ".bright_cyan())?;
                        }
                    }
                }
            }
            Node::TableAlign(_) => {
                // Already handled in the TableCell last_cell_in_row logic
            }
            Node::TableRow(row) => {
                render_table_row(row, &column_widths, highlighter, &config, writer)?;
            }
            _ => {}
        }
    }

    // Render bottom border
    render_table_bottom_border(&column_widths, col_count, writer)?;

    writeln!(writer)?;
    Ok(())
}

/// Calculate column widths for a table
fn calculate_column_widths(nodes: &[Node]) -> Vec<usize> {
    let mut column_widths: Vec<usize> = Vec::new();

    for node in nodes {
        match node {
            Node::TableRow(row) => {
                for (col_idx, cell_node) in row.values.iter().enumerate() {
                    if let Node::TableCell(cell) = cell_node {
                        let content = render_inline_content(&cell.values);
                        let width = visible_width(&content);

                        if col_idx >= column_widths.len() {
                            column_widths.resize(col_idx + 1, 0);
                        }
                        column_widths[col_idx] = column_widths[col_idx].max(width);
                    }
                }
            }
            Node::TableCell(cell) => {
                let content = render_inline_content(&cell.values);
                let width = visible_width(&content);

                if cell.column >= column_widths.len() {
                    column_widths.resize(cell.column + 1, 0);
                }
                column_widths[cell.column] = column_widths[cell.column].max(width);
            }
            _ => {}
        }
    }

    column_widths
}

/// Render table top border
fn render_table_top_border<W: Write>(
    column_widths: &[usize],
    col_count: usize,
    writer: &mut W,
) -> io::Result<()> {
    write!(writer, "{}", "┌".bright_black())?;
    for i in 0..col_count {
        let width = column_widths.get(i).copied().unwrap_or(4);
        write!(writer, "{}", "─".repeat(width + 2).bright_black())?;
        if i < col_count - 1 {
            write!(writer, "{}", "┬".bright_black())?;
        }
    }
    writeln!(writer, "{}", "┐".bright_black())?;
    Ok(())
}

/// Render table bottom border
fn render_table_bottom_border<W: Write>(
    column_widths: &[usize],
    col_count: usize,
    writer: &mut W,
) -> io::Result<()> {
    write!(writer, "{}", "└".bright_black())?;
    for i in 0..col_count {
        let width = column_widths.get(i).copied().unwrap_or(4);
        write!(writer, "{}", "─".repeat(width + 2).bright_black())?;
        if i < col_count - 1 {
            write!(writer, "{}", "┴".bright_black())?;
        }
    }
    writeln!(writer, "{}", "┘".bright_black())?;
    Ok(())
}

/// Render table header with alignment and column widths
fn render_table_header<W: Write>(
    header: &mq_markdown::TableAlign,
    column_widths: &[usize],
    writer: &mut W,
) -> io::Result<()> {
    write!(writer, "{}", "├".bright_black())?;
    for (i, align) in header.align.iter().enumerate() {
        let width = column_widths.get(i).copied().unwrap_or(4);
        let (left, right) = match align {
            mq_markdown::TableAlignKind::Left => (":", "─"),
            mq_markdown::TableAlignKind::Right => ("─", ":"),
            mq_markdown::TableAlignKind::Center => (":", ":"),
            mq_markdown::TableAlignKind::None => ("─", "─"),
        };

        write!(writer, "{}", left.bright_black())?;
        write!(writer, "{}", "─".repeat(width).bright_black())?;
        write!(writer, "{}", right.bright_black())?;

        if i < header.align.len() - 1 {
            write!(writer, "{}", "┼".bright_black())?;
        }
    }
    writeln!(writer, "{}", "┤".bright_black())?;
    Ok(())
}

/// Render table row with column widths
fn render_table_row<W: Write>(
    row: &mq_markdown::TableRow,
    column_widths: &[usize],
    highlighter: &mut SyntaxHighlighter,
    config: &RenderConfig,
    writer: &mut W,
) -> io::Result<()> {
    write!(writer, "{}", "│ ".bright_cyan())?;
    for (col_idx, cell_node) in row.values.iter().enumerate() {
        if let Node::TableCell(cell) = cell_node {
            let content = render_inline_content(&cell.values);
            let width = column_widths.get(col_idx).copied().unwrap_or(0);

            for value in &cell.values {
                render_node_inline(value, 0, true, highlighter, config, writer)?;
            }

            // Pad with spaces to align columns
            let content_width = visible_width(&content);
            if content_width < width {
                write!(writer, "{}", " ".repeat(width - content_width))?;
            }

            write!(writer, " {}", "│ ".bright_cyan())?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

/// Render table cell with column width
fn render_table_cell<W: Write>(
    cell: &mq_markdown::TableCell,
    column_widths: &[usize],
    highlighter: &mut SyntaxHighlighter,
    config: &RenderConfig,
    writer: &mut W,
) -> io::Result<()> {
    write!(writer, "{}", "│ ".bright_cyan())?;

    let content = render_inline_content(&cell.values);
    let width = column_widths.get(cell.column).copied().unwrap_or(0);

    for value in &cell.values {
        render_node_inline(value, 0, true, highlighter, config, writer)?;
    }

    // Pad with spaces to align columns
    let content_width = visible_width(&content);
    if content_width < width {
        write!(writer, "{}", " ".repeat(width - content_width))?;
    }

    write!(writer, " ")?;
    writeln!(writer, "{}", "│".bright_cyan())?;
    Ok(())
}

/// Render an image to the terminal if possible
fn render_image_to_terminal(path: &str) -> io::Result<()> {
    // Check if the path is a local file
    if path.starts_with("http://") || path.starts_with("https://") {
        // For remote images, we would need to download them first
        // For now, skip rendering remote images
        return Ok(());
    }

    let image_path = Path::new(path);
    if !image_path.exists() {
        return Ok(());
    }

    // Use viuer to display the image with default configuration
    // This will auto-detect the best protocol (Kitty, iTerm2, Sixel, or blocks)
    let conf = viuer::Config {
        width: Some(60),
        height: None,
        absolute_offset: false,
        ..Default::default()
    };

    // Try to open and display the image
    if let Ok(img) = image::open(path) {
        let _ = viuer::print(&img, &conf);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_markdown::{Markdown, Node};

    #[test]
    fn test_render_markdown_to_string_simple_text() {
        let markdown: Markdown = "Hello World".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Hello World"));
    }

    #[test]
    fn test_render_markdown_to_string_heading() {
        let markdown: Markdown = "# Heading 1\n## Heading 2\n### Heading 3\n#### Heading 4\n##### Heading 5\n###### Heading 6\n".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Heading 1"));
        assert!(result.contains("Heading 2"));
        assert!(result.contains("Heading 3"));
        assert!(result.contains("Heading 4"));
        assert!(result.contains("Heading 5"));
        assert!(result.contains("Heading 6"));
    }

    #[test]
    fn test_heading_full_width_highlight_padding_accounts_for_symbol_and_links() {
        // Regression test: the full-width background bar must reach exactly
        // `WIDTH` visible columns regardless of heading depth (the "▶"
        // marker is repeated per level, so its width isn't always 1) and
        // regardless of embedded OSC 8 hyperlink escapes inflating the raw
        // string length without affecting what's actually printed.
        let markdown: Markdown = "## [Linked Heading](https://example.com)".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        let line = result.lines().find(|l| !l.trim().is_empty()).unwrap();
        assert_eq!(visible_width(line), *WIDTH);
    }

    #[test]
    fn test_render_markdown_to_string_list() {
        let markdown: Markdown = "- Item 1\n- Item 2\n- Item 3".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Item 1"));
        assert!(result.contains("Item 2"));
        assert!(result.contains("Item 3"));
    }

    #[test]
    fn test_render_markdown_to_string_code_block() {
        let markdown: Markdown = "```rust\nfn main() {}\n```".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        // Code blocks may be syntax highlighted, so just check for the function name
        assert!(result.contains("main"));
    }

    #[test]
    fn test_render_markdown_to_string_inline_code() {
        let markdown: Markdown = "This is `inline code` text".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("inline code"));
    }

    #[test]
    fn test_render_markdown_to_string_bold() {
        let markdown: Markdown = "This is **bold** text".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("bold"));
    }

    #[test]
    fn test_render_markdown_to_string_italic() {
        let markdown: Markdown = "This is *italic* text".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("italic"));
    }

    #[test]
    fn test_render_markdown_to_string_link() {
        let markdown: Markdown = "[Link Text](https://example.com)".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Link Text"));
    }

    #[test]
    fn test_render_markdown_to_string_blockquote() {
        let markdown: Markdown = "> This is a quote".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("This is a quote"));
    }

    #[test]
    fn test_render_markdown_to_string_horizontal_rule() {
        let markdown: Markdown = "---".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        // Check that some separator is rendered
        assert!(!result.is_empty());
    }

    #[test]
    fn test_detect_callout_note() {
        assert!(detect_callout("[!NOTE] Test").is_some());
    }

    #[test]
    fn test_detect_callout_tip() {
        assert!(detect_callout("[!TIP] Test").is_some());
    }

    #[test]
    fn test_detect_callout_important() {
        assert!(detect_callout("[!IMPORTANT] Test").is_some());
    }

    #[test]
    fn test_detect_callout_warning() {
        assert!(detect_callout("[!WARNING] Test").is_some());
    }

    #[test]
    fn test_detect_callout_caution() {
        assert!(detect_callout("[!CAUTION] Test").is_some());
    }

    #[test]
    fn test_detect_callout_case_insensitive() {
        assert!(detect_callout("[!note] Test").is_some());
        assert!(detect_callout("[!Note] Test").is_some());
    }

    #[test]
    fn test_detect_callout_none() {
        assert!(detect_callout("Regular text").is_none());
        assert!(detect_callout("[NOTE] No exclamation").is_none());
    }

    #[test]
    fn test_make_clickable_link() {
        let link = make_clickable_link("https://example.com", "Example");
        assert!(link.contains("https://example.com"));
        assert!(link.contains("Example"));
    }

    #[test]
    fn test_render_inline_content_text() {
        let nodes = vec![Node::Text(mq_markdown::Text {
            value: "Hello".to_string(),
            position: None,
        })];
        let result = render_inline_content(&nodes);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_render_inline_content_inline_code() {
        let nodes = vec![Node::CodeInline(mq_markdown::CodeInline {
            value: "code".into(),
            position: None,
        })];
        let result = render_inline_content(&nodes);
        assert_eq!(result, "`code`");
    }

    #[test]
    fn test_render_inline_content_strong() {
        let nodes = vec![Node::Strong(mq_markdown::Strong {
            values: vec![Node::Text(mq_markdown::Text {
                value: "bold".to_string(),
                position: None,
            })],
            position: None,
        })];
        let result = render_inline_content(&nodes);
        assert_eq!(result, "bold");
    }

    #[test]
    fn test_render_inline_content_emphasis() {
        let nodes = vec![Node::Emphasis(mq_markdown::Emphasis {
            values: vec![Node::Text(mq_markdown::Text {
                value: "italic".to_string(),
                position: None,
            })],
            position: None,
        })];
        let result = render_inline_content(&nodes);
        assert_eq!(result, "italic");
    }

    #[test]
    fn test_needs_space_before() {
        // Test with actual parsed markdown to avoid manual construction
        let markdown: Markdown = "[link](url) **bold** *italic* `code` text".parse().unwrap();

        // Extract nodes from parsed markdown
        if let Some(Node::Fragment(fragment)) = markdown.nodes.first() {
            for node in &fragment.values {
                match node {
                    Node::Link(_) => assert!(needs_space_before(node)),
                    Node::Strong(_) => assert!(needs_space_before(node)),
                    Node::Emphasis(_) => assert!(needs_space_before(node)),
                    Node::CodeInline(_) => assert!(needs_space_before(node)),
                    Node::Text(_) => assert!(!needs_space_before(node)),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn test_calculate_column_widths() {
        let nodes = vec![
            Node::TableCell(mq_markdown::TableCell {
                values: vec![Node::Text(mq_markdown::Text {
                    value: "Short".to_string(),
                    position: None,
                })],
                column: 0,
                row: 0,
                position: None,
            }),
            Node::TableCell(mq_markdown::TableCell {
                values: vec![Node::Text(mq_markdown::Text {
                    value: "Very Long Text".to_string(),
                    position: None,
                })],
                column: 1,
                row: 0,
                position: None,
            }),
        ];
        let widths = calculate_column_widths(&nodes);
        assert_eq!(widths[0], 5); // "Short"
        assert_eq!(widths[1], 14); // "Very Long Text"
    }

    #[test]
    fn test_render_markdown_ordered_list() {
        let markdown: Markdown = "1. First\n2. Second\n3. Third".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("First"));
        assert!(result.contains("Second"));
        assert!(result.contains("Third"));
    }

    #[test]
    fn test_render_markdown_checkbox_list() {
        let markdown: Markdown = "- [x] Done\n- [ ] Todo".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Done"));
        assert!(result.contains("Todo"));
    }

    #[test]
    fn test_render_markdown_table() {
        let markdown: Markdown =
            "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |"
                .parse()
                .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Header 1"));
        assert!(result.contains("Header 2"));
        assert!(result.contains("Cell 1"));
        assert!(result.contains("Cell 2"));
    }

    #[test]
    fn test_render_markdown_nested_list() {
        let markdown: Markdown = "- Item 1\n  - Nested 1\n  - Nested 2\n- Item 2"
            .parse()
            .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Item 1"));
        assert!(result.contains("Nested 1"));
        assert!(result.contains("Nested 2"));
        assert!(result.contains("Item 2"));
    }

    #[test]
    fn test_render_markdown_mixed_formatting() {
        let markdown: Markdown = "**Bold** and *italic* with `code`".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Bold"));
        assert!(result.contains("italic"));
        assert!(result.contains("code"));
    }

    #[test]
    fn test_render_callout_blockquote_note() {
        let markdown: Markdown = "> [!NOTE] This is a note callout\n> Additional info"
            .parse()
            .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        // The callout icon and name should be present
        assert!(result.contains("ℹ️"));
        assert!(result.contains("Note"));
        assert!(result.contains("This is a note callout"));
        assert!(result.contains("Additional info"));
        // Should have box drawing characters
        assert!(result.contains("┌─"));
        assert!(result.contains("└─"));
    }

    #[test]
    fn test_render_callout_blockquote_tip() {
        let markdown: Markdown = "> [!TIP] This is a tip callout".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("💡"));
        assert!(result.contains("Tip"));
        assert!(result.contains("This is a tip callout"));
        assert!(result.contains("┌─"));
        assert!(result.contains("└─"));
    }

    #[test]
    fn test_render_callout_blockquote_important() {
        let markdown: Markdown = "> [!IMPORTANT] Important info".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("❗"));
        assert!(result.contains("Important"));
        assert!(result.contains("Important info"));
        assert!(result.contains("┌─"));
        assert!(result.contains("└─"));
    }

    #[test]
    fn test_render_callout_blockquote_warning() {
        let markdown: Markdown = "> [!WARNING] Warning info".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("⚠️"));
        assert!(result.contains("Warning"));
        assert!(result.contains("Warning info"));
        assert!(result.contains("┌─"));
        assert!(result.contains("└─"));
    }

    #[test]
    fn test_render_callout_blockquote_caution() {
        let markdown: Markdown = "> [!CAUTION] Caution info".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("🔥"));
        assert!(result.contains("Caution"));
        assert!(result.contains("Caution info"));
        assert!(result.contains("┌─"));
        assert!(result.contains("└─"));
    }

    #[test]
    fn test_render_callout_blockquote_case_insensitive() {
        let markdown: Markdown = "> [!note] lower case note\n\n> [!Tip] mixed case tip"
            .parse()
            .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("ℹ️"));
        assert!(result.contains("Note"));
        assert!(result.contains("lower case note"));
        assert!(result.contains("💡"));
        assert!(result.contains("Tip"));
        assert!(result.contains("mixed case tip"));
    }

    #[test]
    fn test_render_markdown_html_block() {
        let markdown: Markdown = "<div>Hello HTML</div>".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        // Should contain the HTML content
        assert!(result.contains("Hello HTML"));
        // Should contain some syntax highlighting (colored output)
        assert!(result.contains("\x1b"));
    }

    #[test]
    fn test_render_markdown_inline_html() {
        let markdown: Markdown = "Text <span>inline html</span> more text".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("inline html"));
        assert!(result.contains("Text"));
        assert!(result.contains("more text"));
    }

    #[test]
    fn test_render_markdown_image_with_alt() {
        let markdown: Markdown = "![Alt text](image.png)".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("🖼️"));
        assert!(result.contains("Alt text"));
        assert!(result.contains("image.png"));
    }

    #[test]
    fn test_render_markdown_image_without_alt() {
        let markdown: Markdown = "![](image.png)".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("🖼️"));
        assert!(result.contains("image.png"));
    }

    #[test]
    fn test_render_markdown_remote_image() {
        let markdown: Markdown = "![Remote](https://example.com/image.png)".parse().unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("🖼️"));
        assert!(result.contains("Remote"));
        assert!(result.contains("https://example.com/image.png"));
    }

    #[test]
    fn test_render_markdown_table_with_alignment() {
        let markdown: Markdown = r#"
| Left | Center | Right |
|:-----|:------:|------:|
| L1   | C1     | R1    |
| L2   | C2     | R2    |
"#
        .parse()
        .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Left"));
        assert!(result.contains("Center"));
        assert!(result.contains("Right"));
        assert!(result.contains("L1"));
        assert!(result.contains("C1"));
        assert!(result.contains("R1"));
        assert!(result.contains("L2"));
        assert!(result.contains("C2"));
        assert!(result.contains("R2"));
        // Check for alignment markers in header border
        assert!(result.contains(":"));
    }

    #[test]
    fn test_render_markdown_table_with_inline_formatting() {
        let markdown: Markdown = r#"
| **Bold** | *Italic* | `Code` |
|----------|----------|--------|
| A        | B        | C      |
"#
        .parse()
        .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Bold"));
        assert!(result.contains("Italic"));
        assert!(result.contains("Code"));
        assert!(result.contains("A"));
        assert!(result.contains("B"));
        assert!(result.contains("C"));
    }

    #[test]
    fn test_render_markdown_table_with_links_and_images() {
        let markdown: Markdown = r#"
| Link | Image |
|------|-------|
| [Google](https://google.com) | ![Alt](img.png) |
"#
        .parse()
        .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Google"));
        assert!(result.contains("https://google.com"));
        assert!(result.contains("🖼️"));
        assert!(result.contains("Alt"));
        assert!(result.contains("img.png"));
    }

    #[test]
    fn test_render_markdown_table_empty_cells() {
        let markdown: Markdown = r#"
| A | B | C |
|---|---|---|
|   | 1 |   |
| 2 |   | 3 |
"#
        .parse()
        .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("A"));
        assert!(result.contains("B"));
        assert!(result.contains("C"));
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
    }

    #[test]
    fn test_render_markdown_table_with_multiple_rows_and_columns() {
        let markdown: Markdown = r#"
| Col1 | Col2 | Col3 | Col4 |
|------|------|------|------|
| A    | B    | C    | D    |
| E    | F    | G    | H    |
| I    | J    | K    | L    |
"#
        .parse()
        .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        for val in &[
            "Col1", "Col2", "Col3", "Col4", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K",
            "L",
        ] {
            assert!(result.contains(val));
        }
    }

    #[test]
    fn test_render_markdown_table_with_rowspan_and_colspan_like_content() {
        // Markdown tables do not support rowspan/colspan, but test for cells with multiline content
        let markdown: Markdown = r#"
| Header |
|--------|
| Line 1<br>Line 2 |
"#
        .parse()
        .unwrap();
        let result = render_markdown_to_string(&markdown).unwrap();
        assert!(result.contains("Header"));
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));
    }
}
