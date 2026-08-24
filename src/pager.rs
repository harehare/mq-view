//! Interactive TUI viewer ("pager mode") for richer markdown browsing:
//! scrolling, a heading outline, incremental search, and auto-reload when
//! the source file changes on disk.

use crate::renderer::{HeadingEntry, RenderConfig, render_markdown_with_outline};
use crate::theme::Theme;
use ansi_to_tui::IntoText;
use mq_markdown::Markdown;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    self as crossterm_terminal, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// Convert a theme color (always `TrueColor`) into ratatui's color type.
fn to_ratatui(c: colored::Color) -> Color {
    match c {
        colored::Color::TrueColor { r, g, b } => Color::Rgb(r, g, b),
        _ => Color::Reset,
    }
}

struct Document {
    text: Text<'static>,
    plain_lines: Vec<String>,
    headings: Vec<HeadingEntry>,
    links: Vec<LinkEntry>,
}

impl Document {
    fn load(content: &str, config: &RenderConfig) -> io::Result<Self> {
        let markdown: Markdown = content
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e}")))?;
        // Direct-to-terminal image drawing (viuer) bypasses our writer and
        // would corrupt the alternate-screen buffer, so it's always off here.
        let config = &RenderConfig {
            inline_images: false,
            ..config.clone()
        };
        let (rendered, headings) = render_markdown_with_outline(&markdown, config)?;
        let links = extract_links(&rendered);
        let text = rendered
            .into_text()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let plain_lines = text.lines.iter().map(line_plain_text).collect();
        Ok(Self {
            text,
            plain_lines,
            headings,
            links,
        })
    }

    fn line_count(&self) -> usize {
        self.text.lines.len()
    }
}

fn line_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// A link found in the rendered document.
struct LinkEntry {
    line: usize,
    url: String,
    text: String,
}

/// Scan rendered ANSI text for OSC 8 hyperlinks
/// (`ESC ] 8 ; ; URL ESC \ text ESC ] 8 ; ; ESC \`, as emitted by
/// `renderer::make_clickable_link`) and record where each one lands. Doing
/// this as a post-processing pass over the final string avoids threading a
/// link collector through every render function.
fn extract_links(rendered: &str) -> Vec<LinkEntry> {
    const OSC8_START: &str = "\x1b]8;;";
    const ST: &str = "\x1b\\";
    const OSC8_END: &str = "\x1b]8;;\x1b\\";

    let mut out = Vec::new();
    for (line_idx, line) in rendered.lines().enumerate() {
        let mut rest = line;
        while let Some(start) = rest.find(OSC8_START) {
            let after_marker = &rest[start + OSC8_START.len()..];
            let Some(url_end) = after_marker.find(ST) else {
                break;
            };
            let url = &after_marker[..url_end];
            let after_url = &after_marker[url_end + ST.len()..];
            let Some(text_end) = after_url.find(OSC8_END) else {
                break;
            };
            let text = &after_url[..text_end];
            if !url.is_empty() {
                out.push(LinkEntry {
                    line: line_idx,
                    url: url.to_string(),
                    text: text.to_string(),
                });
            }
            rest = &after_url[text_end + OSC8_END.len()..];
        }
    }
    out
}

/// Best-effort GitHub-style heading slug: lowercase, spaces to hyphens,
/// punctuation stripped. Used to resolve `#anchor` links to a heading.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' {
            out.push('-');
        }
    }
    out
}

struct SearchMatch {
    line: usize,
    start: usize,
    end: usize,
}

fn find_matches(plain_lines: &[String], query: &str) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut matches = Vec::new();
    for (line_idx, line) in plain_lines.iter().enumerate() {
        let haystack = line.to_lowercase();
        let mut cursor = 0;
        while let Some(pos) = haystack[cursor..].find(&needle) {
            let abs = cursor + pos;
            matches.push(SearchMatch {
                line: line_idx,
                start: abs,
                end: abs + needle.len(),
            });
            cursor = abs + needle.len().max(1);
            if cursor >= haystack.len() {
                break;
            }
        }
    }
    matches
}

/// Overlay search-match highlighting on top of a line's existing syntax
/// styling. `ranges` are `(start_byte, end_byte, is_current_match)`.
fn render_line_with_highlights(
    line: &Line<'static>,
    ranges: &[(usize, usize, bool)],
    theme: &Theme,
) -> Line<'static> {
    if ranges.is_empty() {
        return line.clone();
    }

    let mut spans = Vec::new();
    let mut pos = 0usize;
    for span in &line.spans {
        let content = span.content.as_ref();
        let span_start = pos;
        let span_end = pos + content.len();

        let mut cuts = vec![span_start, span_end];
        for &(s, e, _) in ranges {
            if s > span_start && s < span_end {
                cuts.push(s);
            }
            if e > span_start && e < span_end {
                cuts.push(e);
            }
        }
        cuts.sort_unstable();
        cuts.dedup();

        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a == b {
                continue;
            }
            let sub = &content[a - span_start..b - span_start];
            let mut style = span.style;
            if let Some(&(_, _, current)) = ranges.iter().find(|&&(s, e, _)| a >= s && b <= e) {
                style = style
                    .bg(to_ratatui(if current {
                        theme.search_current
                    } else {
                        theme.search_match
                    }))
                    .fg(Color::Black);
            }
            spans.push(Span::styled(sub.to_string(), style));
        }
        pos = span_end;
    }
    Line::from(spans)
}

enum Mode {
    Normal,
    Search,
}

#[derive(Clone, Copy)]
enum StatusKind {
    Info,
    Success,
    Warn,
}

struct Status {
    text: String,
    kind: StatusKind,
}

impl Status {
    fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Info,
        }
    }

    fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Success,
        }
    }

    fn warn(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Warn,
        }
    }
}

struct App {
    doc: Document,
    /// Raw markdown last loaded, so config changes (e.g. line-number
    /// toggle) can re-render without hitting disk.
    content: String,
    config: RenderConfig,
    scroll: usize,
    show_outline: bool,
    outline_state: ListState,
    show_links: bool,
    links_state: ListState,
    /// Popup rects from the last draw, so mouse clicks can hit-test them.
    outline_popup_rect: Option<Rect>,
    links_popup_rect: Option<Rect>,
    mode: Mode,
    search_input: String,
    matches: Vec<SearchMatch>,
    current_match: Option<usize>,
    status: Option<Status>,
    path: Option<PathBuf>,
    title: String,
    mq_query: Option<String>,
    back_stack: Vec<(PathBuf, usize)>,
    forward_stack: Vec<(PathBuf, usize)>,
}

fn max_scroll(doc: &Document, content_height: usize) -> usize {
    doc.line_count().saturating_sub(content_height.max(1))
}

fn clamp_scroll(line: usize, total_lines: usize, content_height: usize) -> usize {
    line.min(total_lines.saturating_sub(content_height.max(1)))
}

/// Index of the heading the current scroll position is "inside" (the last
/// heading at or above the top visible line), falling back to the first
/// heading if we're above it.
fn nearest_heading_index(app: &App) -> Option<usize> {
    app.doc
        .headings
        .iter()
        .rposition(|h| h.line <= app.scroll)
        .or(if app.doc.headings.is_empty() {
            None
        } else {
            Some(0)
        })
}

fn current_section_title(app: &App) -> Option<&str> {
    nearest_heading_index(app).map(|i| app.doc.headings[i].title.as_str())
}

/// Index of the link closest to (at or after) the current scroll position,
/// so opening the link list starts near what's on screen.
fn nearest_link_index(app: &App) -> Option<usize> {
    app.doc
        .links
        .iter()
        .position(|l| l.line >= app.scroll)
        .or(if app.doc.links.is_empty() {
            None
        } else {
            Some(app.doc.links.len() - 1)
        })
}

fn scroll_by(app: &mut App, delta: isize, content_height: usize) {
    let max = max_scroll(&app.doc, content_height);
    app.scroll = (app.scroll as isize + delta).clamp(0, max as isize) as usize;
}

fn outline_move(app: &mut App, delta: isize) {
    let len = app.doc.headings.len();
    if len == 0 {
        return;
    }
    let current = app.outline_state.selected().unwrap_or(0) as isize;
    let next = (current + delta).clamp(0, len as isize - 1);
    app.outline_state.select(Some(next as usize));
}

fn links_move(app: &mut App, delta: isize) {
    let len = app.doc.links.len();
    if len == 0 {
        return;
    }
    let current = app.links_state.selected().unwrap_or(0) as isize;
    let next = (current + delta).clamp(0, len as isize - 1);
    app.links_state.select(Some(next as usize));
}

/// Jump to the heading whose slug matches `anchor` (a `#fragment`, with or
/// without the leading `#`), if any.
fn jump_to_anchor(app: &mut App, anchor: &str, content_height: usize) -> bool {
    let anchor = anchor.trim_start_matches('#');
    if let Some(h) = app
        .doc
        .headings
        .iter()
        .find(|h| slugify(&h.title) == slugify(anchor))
    {
        app.scroll = clamp_scroll(h.line, app.doc.line_count(), content_height);
        true
    } else {
        false
    }
}

fn open_external(url: &str) {
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(target_os = "macos")]
    cmd.arg(url);

    #[cfg(target_os = "linux")]
    let mut cmd = Command::new("xdg-open");
    #[cfg(target_os = "linux")]
    cmd.arg(url);

    #[cfg(target_os = "windows")]
    let mut cmd = Command::new("cmd");
    #[cfg(target_os = "windows")]
    cmd.args(["/C", "start", "", url]);

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let _ = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn();
    }
}

/// Resolve and follow a link: `#anchor` jumps within the current document,
/// `scheme://...`/`mailto:` opens externally, anything else is treated as a
/// local file path relative to the currently open file (or CWD if the
/// document has no backing file, e.g. piped stdin).
fn follow_link(app: &mut App, link_idx: usize, content_height: usize) {
    let Some(link) = app.doc.links.get(link_idx) else {
        return;
    };
    let url = link.url.clone();

    if let Some(anchor) = url.strip_prefix('#') {
        if !jump_to_anchor(app, anchor, content_height) {
            app.status = Some(Status::warn(format!("No heading matches #{anchor}")));
        }
        return;
    }

    if url.contains("://") || url.starts_with("mailto:") {
        open_external(&url);
        app.status = Some(Status::info(format!("Opened {url}")));
        return;
    }

    let (path_part, anchor) = match url.split_once('#') {
        Some((p, a)) => (p, Some(a.to_string())),
        None => (url.as_str(), None),
    };

    let base_dir = app
        .path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let target = base_dir.join(path_part);

    if !target.exists() {
        app.status = Some(Status::warn(format!("Cannot resolve link: {path_part}")));
        return;
    }

    let Ok(content) = std::fs::read_to_string(&target) else {
        app.status = Some(Status::warn(format!("Cannot read {}", target.display())));
        return;
    };
    let Ok(doc) = Document::load(&content, &app.config) else {
        app.status = Some(Status::warn(format!("Cannot render {}", target.display())));
        return;
    };

    if let Some(current_path) = app.path.clone() {
        app.back_stack.push((current_path, app.scroll));
    }
    app.forward_stack.clear();

    app.title = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "?".to_string());
    app.path = Some(target);
    app.content = content;
    app.mq_query = None;
    app.doc = doc;
    app.scroll = 0;
    app.search_input.clear();
    app.matches.clear();
    app.current_match = None;

    if let Some(anchor) = anchor {
        jump_to_anchor(app, &anchor, content_height);
    }
    app.status = Some(Status::success(format!(
        "Opened {}",
        app.path.as_deref().unwrap_or(Path::new("?")).display()
    )));
}

/// Navigate `back_stack`/`forward_stack` in `direction` (-1 = back, +1 =
/// forward), reloading the target file at its saved scroll position.
fn navigate_history(app: &mut App, direction: isize, content_height: usize) {
    let (from, to) = if direction < 0 {
        (&mut app.back_stack, &mut app.forward_stack)
    } else {
        (&mut app.forward_stack, &mut app.back_stack)
    };
    let Some((target_path, target_scroll)) = from.pop() else {
        app.status = Some(Status::warn(if direction < 0 {
            "No earlier page"
        } else {
            "No later page"
        }));
        return;
    };

    let Ok(content) = std::fs::read_to_string(&target_path) else {
        app.status = Some(Status::warn(format!(
            "Cannot read {}",
            target_path.display()
        )));
        return;
    };
    let Ok(doc) = Document::load(&content, &app.config) else {
        app.status = Some(Status::warn(format!(
            "Cannot render {}",
            target_path.display()
        )));
        return;
    };

    if let Some(current_path) = app.path.clone() {
        to.push((current_path, app.scroll));
    }

    app.title = target_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "?".to_string());
    app.path = Some(target_path);
    app.content = content;
    app.doc = doc;
    app.scroll = clamp_scroll(target_scroll, app.doc.line_count(), content_height);
    app.search_input.clear();
    app.matches.clear();
    app.current_match = None;
}

fn jump_to_match(app: &mut App, content_height: usize) {
    if let Some(m) = app.current_match.and_then(|i| app.matches.get(i)) {
        app.scroll = clamp_scroll(m.line, app.doc.line_count(), content_height);
    }
}

fn cycle_match(app: &mut App, delta: isize, content_height: usize) {
    if app.matches.is_empty() {
        app.status = Some(Status::warn("No search pattern"));
        return;
    }
    let len = app.matches.len() as isize;
    let current = app.current_match.map_or(-1, |i| i as isize);
    let next = (current + delta).rem_euclid(len);
    app.current_match = Some(next as usize);
    jump_to_match(app, content_height);
    app.status = Some(Status::info(format!("match {}/{}", next + 1, len)));
}

/// Returns `true` if the app should quit.
/// What the event loop should do after a key press. Following a link (or
/// navigating history) can change the file being displayed, which means
/// the file watcher needs to be pointed at a new path.
enum KeyOutcome {
    Continue,
    Quit,
    RespawnWatcher,
}

fn rerender(app: &mut App, content_height: usize) {
    match Document::load(&app.content, &app.config) {
        Ok(doc) => {
            app.doc = doc;
            app.matches = find_matches(&app.doc.plain_lines, &app.search_input);
            app.current_match = None;
            app.scroll = app.scroll.min(max_scroll(&app.doc, content_height));
        }
        Err(e) => {
            app.status = Some(Status::warn(format!("Render failed: {e}")));
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent, content_height: usize) -> KeyOutcome {
    if let Mode::Search = app.mode {
        match key.code {
            KeyCode::Esc => {
                app.mode = Mode::Normal;
                app.search_input.clear();
            }
            KeyCode::Enter => {
                app.mode = Mode::Normal;
                app.matches = find_matches(&app.doc.plain_lines, &app.search_input);
                if app.matches.is_empty() {
                    app.current_match = None;
                    app.status = Some(Status::warn(format!(
                        "Pattern not found: {}",
                        app.search_input
                    )));
                } else {
                    let idx = app
                        .matches
                        .iter()
                        .position(|m| m.line >= app.scroll)
                        .unwrap_or(0);
                    app.current_match = Some(idx);
                    jump_to_match(app, content_height);
                    app.status = Some(Status::info(format!(
                        "match {}/{}",
                        idx + 1,
                        app.matches.len()
                    )));
                }
            }
            KeyCode::Backspace => {
                app.search_input.pop();
            }
            KeyCode::Char(c) => app.search_input.push(c),
            _ => {}
        }
        return KeyOutcome::Continue;
    }

    if app.show_outline {
        match key.code {
            KeyCode::Esc | KeyCode::Tab => app.show_outline = false,
            KeyCode::Up | KeyCode::Char('k') => outline_move(app, -1),
            KeyCode::Down | KeyCode::Char('j') => outline_move(app, 1),
            KeyCode::Enter => {
                if let Some(h) = app
                    .outline_state
                    .selected()
                    .and_then(|i| app.doc.headings.get(i))
                {
                    app.scroll = clamp_scroll(h.line, app.doc.line_count(), content_height);
                }
                app.show_outline = false;
            }
            KeyCode::Char('q') => return KeyOutcome::Quit,
            _ => {}
        }
        return KeyOutcome::Continue;
    }

    if app.show_links {
        match key.code {
            KeyCode::Esc => app.show_links = false,
            KeyCode::Up | KeyCode::Char('k') => links_move(app, -1),
            KeyCode::Down | KeyCode::Char('j') => links_move(app, 1),
            KeyCode::Enter => {
                if let Some(idx) = app.links_state.selected() {
                    app.show_links = false;
                    let had_path = app.path.clone();
                    follow_link(app, idx, content_height);
                    if app.path != had_path {
                        return KeyOutcome::RespawnWatcher;
                    }
                }
            }
            KeyCode::Char('q') => return KeyOutcome::Quit,
            _ => {}
        }
        return KeyOutcome::Continue;
    }

    app.status = None;
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return KeyOutcome::Quit,
        KeyCode::Char('j') | KeyCode::Down => scroll_by(app, 1, content_height),
        KeyCode::Char('k') | KeyCode::Up => scroll_by(app, -1, content_height),
        KeyCode::Char(' ') | KeyCode::PageDown | KeyCode::Char('f') => {
            scroll_by(app, content_height as isize, content_height)
        }
        KeyCode::PageUp | KeyCode::Char('b') => {
            scroll_by(app, -(content_height as isize), content_height)
        }
        KeyCode::Char('d') => scroll_by(app, (content_height / 2) as isize, content_height),
        KeyCode::Char('u') => scroll_by(app, -((content_height as isize) / 2), content_height),
        KeyCode::Char('g') | KeyCode::Home => app.scroll = 0,
        KeyCode::Char('G') | KeyCode::End => app.scroll = max_scroll(&app.doc, content_height),
        KeyCode::Tab => {
            app.show_outline = true;
            app.outline_state.select(nearest_heading_index(app));
        }
        KeyCode::Enter => {
            if app.doc.links.is_empty() {
                app.status = Some(Status::warn("No links in this document"));
            } else {
                app.show_links = true;
                app.links_state.select(nearest_link_index(app));
            }
        }
        KeyCode::Char('/') => {
            app.mode = Mode::Search;
            app.search_input.clear();
        }
        KeyCode::Char('n') => cycle_match(app, 1, content_height),
        KeyCode::Char('N') => cycle_match(app, -1, content_height),
        KeyCode::Char('L') => {
            app.config.line_numbers = !app.config.line_numbers;
            rerender(app, content_height);
        }
        KeyCode::Char('[') => {
            let had_path = app.path.clone();
            navigate_history(app, -1, content_height);
            if app.path != had_path {
                return KeyOutcome::RespawnWatcher;
            }
        }
        KeyCode::Char(']') => {
            let had_path = app.path.clone();
            navigate_history(app, 1, content_height);
            if app.path != had_path {
                return KeyOutcome::RespawnWatcher;
            }
        }
        _ => {}
    }
    KeyOutcome::Continue
}

/// Index of the list row a click landed on, given the popup's outer `Rect`
/// (as drawn by `draw_list_popup`, which borders the list on all sides).
fn list_item_at(rect: Rect, row: u16) -> Option<usize> {
    if row <= rect.y || row >= rect.y + rect.height.saturating_sub(1) {
        return None;
    }
    Some((row - rect.y - 1) as usize)
}

fn handle_mouse(app: &mut App, me: MouseEvent, content_height: usize) -> KeyOutcome {
    let pos = Position::new(me.column, me.row);
    match me.kind {
        MouseEventKind::ScrollDown => {
            if app.show_outline {
                outline_move(app, 1);
            } else if app.show_links {
                links_move(app, 1);
            } else {
                scroll_by(app, 3, content_height);
            }
        }
        MouseEventKind::ScrollUp => {
            if app.show_outline {
                outline_move(app, -1);
            } else if app.show_links {
                links_move(app, -1);
            } else {
                scroll_by(app, -3, content_height);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if app.show_outline
                && let Some(rect) = app.outline_popup_rect
                && rect.contains(pos)
                && let Some(idx) = list_item_at(rect, me.row)
                && idx < app.doc.headings.len()
            {
                let h = &app.doc.headings[idx];
                app.scroll = clamp_scroll(h.line, app.doc.line_count(), content_height);
                app.show_outline = false;
            } else if app.show_links
                && let Some(rect) = app.links_popup_rect
                && rect.contains(pos)
                && let Some(idx) = list_item_at(rect, me.row)
                && idx < app.doc.links.len()
            {
                app.show_links = false;
                let had_path = app.path.clone();
                follow_link(app, idx, content_height);
                if app.path != had_path {
                    return KeyOutcome::RespawnWatcher;
                }
            }
        }
        _ => {}
    }
    KeyOutcome::Continue
}

fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let title_area = rows[0];
    let body_area = rows[1];
    let footer_area = rows[2];

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(body_area);
    let content_area = cols[0];
    let scrollbar_area = cols[1];

    let height = content_area.height as usize;
    let total = app.doc.line_count();
    let start = app.scroll.min(total);
    let end = (start + height).min(total);

    let theme = app.config.theme;
    let visible_lines: Vec<Line> = app.doc.text.lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, line)| {
            let line_idx = start + offset;
            let ranges: Vec<(usize, usize, bool)> = app
                .matches
                .iter()
                .enumerate()
                .filter(|(_, m)| m.line == line_idx)
                .map(|(i, m)| (m.start, m.end, Some(i) == app.current_match))
                .collect();
            render_line_with_highlights(line, &ranges, &theme)
        })
        .collect();

    frame.render_widget(Paragraph::new(Text::from(visible_lines)), content_area);
    draw_title(frame, title_area, app);
    draw_scrollbar(frame, scrollbar_area, total, start, &theme);
    draw_footer(frame, footer_area, app, height);

    if app.show_outline {
        let items: Vec<String> = app
            .doc
            .headings
            .iter()
            .map(|h| {
                format!(
                    "{}{}",
                    "  ".repeat(h.depth.saturating_sub(1) as usize),
                    h.title
                )
            })
            .collect();
        let title = format!(" Outline ({}) ", items.len());
        app.outline_popup_rect = Some(draw_list_popup(
            frame,
            area,
            &title,
            &items,
            &mut app.outline_state,
            &theme,
        ));
    } else {
        app.outline_popup_rect = None;
    }

    if app.show_links {
        let items: Vec<String> = app
            .doc
            .links
            .iter()
            .map(|l| {
                let text = if l.text.trim().is_empty() {
                    l.url.clone()
                } else {
                    l.text.clone()
                };
                format!("{text}  ({})", l.url)
            })
            .collect();
        let title = format!(" Links ({}) ", items.len());
        app.links_popup_rect = Some(draw_list_popup(
            frame,
            area,
            &title,
            &items,
            &mut app.links_state,
            &theme,
        ));
    } else {
        app.links_popup_rect = None;
    }
}

fn draw_title(frame: &mut Frame, area: Rect, app: &App) {
    let style = Style::default()
        .fg(Color::Black)
        .bg(to_ratatui(app.config.theme.ui_accent))
        .add_modifier(Modifier::BOLD);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Percentage(40)])
        .split(area);
    frame.render_widget(
        Paragraph::new(format!(" mq-view — {}", app.title)).style(style),
        cols[0],
    );
    let section = current_section_title(app).unwrap_or_default();
    frame.render_widget(
        Paragraph::new(format!("{section} "))
            .style(style)
            .alignment(Alignment::Right),
        cols[1],
    );
}

fn draw_scrollbar(frame: &mut Frame, area: Rect, total: usize, position: usize, theme: &Theme) {
    let mut state = ScrollbarState::new(total).position(position);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_style(Style::default().fg(to_ratatui(theme.ui_muted)))
        .thumb_style(Style::default().fg(to_ratatui(theme.ui_accent)));
    frame.render_stateful_widget(scrollbar, area, &mut state);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, content_height: usize) {
    match &app.mode {
        Mode::Search => {
            let style = Style::default()
                .fg(Color::Black)
                .bg(to_ratatui(app.config.theme.ui_accent))
                .add_modifier(Modifier::BOLD);
            frame.render_widget(
                Paragraph::new(format!(" /{}", app.search_input)).style(style),
                area,
            );
        }
        Mode::Normal => {
            if let Some(status) = &app.status {
                let theme = &app.config.theme;
                let bg = match status.kind {
                    StatusKind::Info => theme.ui_accent,
                    StatusKind::Success => theme.callout[1], // Tip's green
                    StatusKind::Warn => theme.callout[3],    // Warning's yellow/amber
                };
                let style = Style::default()
                    .fg(Color::Black)
                    .bg(to_ratatui(bg))
                    .add_modifier(Modifier::BOLD);
                frame.render_widget(
                    Paragraph::new(format!(" {}", status.text)).style(style),
                    area,
                );
            } else {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(1), Constraint::Length(6)])
                    .split(area);
                frame.render_widget(
                    Paragraph::new(
                        " q:quit  j/k:scroll  f/b/d/u:page  g/G:top/bottom  Tab:outline  Enter:links  [/]:back/fwd  L:line#  /:search  n/N:next/prev",
                    )
                    .style(Style::default().fg(to_ratatui(app.config.theme.ui_muted))),
                    cols[0],
                );
                let max = max_scroll(&app.doc, content_height);
                let pct = (app.scroll * 100).checked_div(max).unwrap_or(100).min(100);
                frame.render_widget(
                    Paragraph::new(format!("{pct}% "))
                        .style(Style::default().fg(to_ratatui(app.config.theme.ui_muted)))
                        .alignment(Alignment::Right),
                    cols[1],
                );
            }
        }
    }
}

/// Draw a bordered, centered list popup (used for both the heading outline
/// and the link list) and return its outer `Rect` so callers can hit-test
/// mouse clicks against it.
fn draw_list_popup(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[String],
    state: &mut ListState,
    theme: &Theme,
) -> Rect {
    let popup = centered_rect(60, 70, area);
    let list_items: Vec<ListItem> = items.iter().map(|s| ListItem::new(s.clone())).collect();
    let accent = to_ratatui(theme.ui_accent);
    let list = List::new(list_items)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(accent))
                .title(title.to_string())
                .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    frame.render_widget(Clear, popup);
    frame.render_stateful_widget(list, popup, state);
    popup
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn spawn_watcher(path: &Path) -> notify::Result<(RecommendedWatcher, Receiver<()>)> {
    let (tx, rx) = channel();
    let file_name = path.file_name().map(|n| n.to_os_string());
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let matches = file_name
                .as_deref()
                .is_none_or(|name| event.paths.iter().any(|p| p.file_name() == Some(name)));
            if matches {
                let _ = tx.send(());
            }
        }
    })?;
    let watch_dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    watcher.watch(watch_dir, RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut watcher: Option<(RecommendedWatcher, Receiver<()>)>,
) -> io::Result<()> {
    loop {
        let (_, term_height) = crossterm_terminal::size()?;
        // Title bar (1 row) + footer (1 row) surround the scrollable body.
        let content_height = term_height.saturating_sub(2).max(1) as usize;

        if let Some((_, rx)) = &watcher
            && rx.try_iter().last().is_some()
        {
            std::thread::sleep(Duration::from_millis(80));
            while rx.try_recv().is_ok() {}
            reload(app, content_height);
        }

        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(200))? {
            let outcome = match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    Some(handle_key(app, key, content_height))
                }
                Event::Mouse(me) => Some(handle_mouse(app, me, content_height)),
                _ => None,
            };
            match outcome {
                Some(KeyOutcome::Quit) => return Ok(()),
                Some(KeyOutcome::RespawnWatcher) => {
                    watcher = app.path.as_deref().and_then(|p| spawn_watcher(p).ok());
                }
                Some(KeyOutcome::Continue) | None => {}
            }
        }
    }
}

fn reload(app: &mut App, content_height: usize) {
    let Some(path) = app.path.clone() else {
        return;
    };
    let result = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|content| match &app.mq_query {
            Some(query) => crate::apply_query(&content, query).map_err(|e| e.to_string()),
            None => Ok(content),
        })
        .and_then(|content| {
            Document::load(&content, &app.config)
                .map(|doc| (content, doc))
                .map_err(|e| e.to_string())
        });
    match result {
        Ok((content, doc)) => {
            app.content = content;
            app.doc = doc;
            app.matches = find_matches(&app.doc.plain_lines, &app.search_input);
            app.current_match = None;
            app.scroll = app.scroll.min(max_scroll(&app.doc, content_height));
            app.status = Some(Status::success("Reloaded"));
        }
        Err(e) => {
            app.status = Some(Status::warn(format!("Reload failed: {e}")));
        }
    }
}

/// Run the interactive pager over `content`. When `path` is given, the file
/// is watched and the view auto-reloads on changes; without a path (e.g.
/// piped stdin) the document is static. `mq_query`, if given, is re-applied
/// on every reload as well as the initial load.
pub fn run_pager(
    content: &str,
    path: Option<PathBuf>,
    config: &RenderConfig,
    mq_query: Option<String>,
) -> io::Result<()> {
    let doc = Document::load(content, config)?;

    // Make sure a panic mid-render doesn't leave the user's terminal stuck
    // in raw mode / the alternate screen.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    let watcher = path.as_deref().and_then(|p| spawn_watcher(p).ok());
    let title = path
        .as_deref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "stdin".to_string());

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        doc,
        content: content.to_string(),
        config: config.clone(),
        scroll: 0,
        show_outline: false,
        outline_state: ListState::default(),
        show_links: false,
        links_state: ListState::default(),
        outline_popup_rect: None,
        links_popup_rect: None,
        mode: Mode::Normal,
        search_input: String::new(),
        matches: Vec::new(),
        current_match: None,
        status: None,
        path,
        title,
        mq_query,
        back_stack: Vec::new(),
        forward_stack: Vec::new(),
    };

    let result = event_loop(&mut terminal, &mut app, watcher);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    // Drop back to the default hook now that the terminal is restored.
    let _ = std::panic::take_hook();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_links_finds_url_and_text() {
        let rendered = format!(
            "before {}text{}\nafter",
            "\x1b]8;;https://example.com\x1b\\", "\x1b]8;;\x1b\\"
        );
        let links = extract_links(&rendered);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].line, 0);
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[0].text, "text");
    }

    #[test]
    fn extract_links_finds_multiple_on_different_lines() {
        let mk = |url: &str, text: &str| format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\");
        let rendered = format!(
            "{}\nsome text\n{}",
            mk("https://a.example", "a"),
            mk("./b.md", "b")
        );
        let links = extract_links(&rendered);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].line, 0);
        assert_eq!(links[0].url, "https://a.example");
        assert_eq!(links[1].line, 2);
        assert_eq!(links[1].url, "./b.md");
    }

    #[test]
    fn extract_links_ignores_lines_without_osc8() {
        assert!(extract_links("plain text\nno links here").is_empty());
    }

    #[test]
    fn slugify_matches_github_style() {
        assert_eq!(slugify("Getting Started"), "getting-started");
        assert_eq!(slugify("mq Query Filtering!"), "mq-query-filtering");
        assert_eq!(slugify("  Trim Me  "), "trim-me");
    }

    #[test]
    fn list_item_at_maps_rows_inside_border_to_indices() {
        let rect = Rect::new(0, 5, 20, 5); // rows 5..10, border at 5 and 9
        assert_eq!(list_item_at(rect, 5), None); // top border
        assert_eq!(list_item_at(rect, 6), Some(0));
        assert_eq!(list_item_at(rect, 7), Some(1));
        assert_eq!(list_item_at(rect, 8), Some(2));
        assert_eq!(list_item_at(rect, 9), None); // bottom border
    }
}
