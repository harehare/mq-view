//! Interactive TUI viewer ("pager mode") for richer markdown browsing:
//! scrolling, a heading outline, incremental search, and auto-reload when
//! the source file changes on disk.

use crate::renderer::{HeadingEntry, RenderConfig, render_markdown_with_outline};
use ansi_to_tui::IntoText;
use mq_markdown::Markdown;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    self as crossterm_terminal, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

struct Document {
    text: Text<'static>,
    plain_lines: Vec<String>,
    headings: Vec<HeadingEntry>,
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
        let text = rendered
            .into_text()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let plain_lines = text.lines.iter().map(line_plain_text).collect();
        Ok(Self {
            text,
            plain_lines,
            headings,
        })
    }

    fn line_count(&self) -> usize {
        self.text.lines.len()
    }
}

fn line_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
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
                    .bg(if current {
                        Color::Rgb(255, 140, 0)
                    } else {
                        Color::Yellow
                    })
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
    scroll: usize,
    show_outline: bool,
    outline_state: ListState,
    mode: Mode,
    search_input: String,
    matches: Vec<SearchMatch>,
    current_match: Option<usize>,
    status: Option<Status>,
    path: Option<PathBuf>,
    title: String,
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
fn handle_key(app: &mut App, key: KeyEvent, content_height: usize) -> bool {
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
        return false;
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
            KeyCode::Char('q') => return true,
            _ => {}
        }
        return false;
    }

    app.status = None;
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return true,
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
        KeyCode::Char('/') => {
            app.mode = Mode::Search;
            app.search_input.clear();
        }
        KeyCode::Char('n') => cycle_match(app, 1, content_height),
        KeyCode::Char('N') => cycle_match(app, -1, content_height),
        _ => {}
    }
    false
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
            render_line_with_highlights(line, &ranges)
        })
        .collect();

    frame.render_widget(Paragraph::new(Text::from(visible_lines)), content_area);
    draw_title(frame, title_area, app);
    draw_scrollbar(frame, scrollbar_area, total, start);
    draw_footer(frame, footer_area, app, height);

    if app.show_outline {
        draw_outline(frame, area, app);
    }
}

fn draw_title(frame: &mut Frame, area: Rect, app: &App) {
    let style = Style::default()
        .fg(Color::Black)
        .bg(Color::Blue)
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

fn draw_scrollbar(frame: &mut Frame, area: Rect, total: usize, position: usize) {
    let mut state = ScrollbarState::new(total).position(position);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_style(Style::default().fg(Color::DarkGray))
        .thumb_style(Style::default().fg(Color::Cyan));
    frame.render_stateful_widget(scrollbar, area, &mut state);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, content_height: usize) {
    match &app.mode {
        Mode::Search => {
            let style = Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
            frame.render_widget(
                Paragraph::new(format!(" /{}", app.search_input)).style(style),
                area,
            );
        }
        Mode::Normal => {
            if let Some(status) = &app.status {
                let bg = match status.kind {
                    StatusKind::Info => Color::Cyan,
                    StatusKind::Success => Color::Green,
                    StatusKind::Warn => Color::Yellow,
                };
                let style = Style::default()
                    .fg(Color::Black)
                    .bg(bg)
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
                        " q:quit  j/k:scroll  f/b/d/u:page  g/G:top/bottom  Tab:outline  /:search  n/N:next/prev match",
                    )
                    .style(Style::default().fg(Color::DarkGray)),
                    cols[0],
                );
                let max = max_scroll(&app.doc, content_height);
                let pct = (app.scroll * 100).checked_div(max).unwrap_or(100).min(100);
                frame.render_widget(
                    Paragraph::new(format!("{pct}% "))
                        .style(Style::default().fg(Color::Gray))
                        .alignment(Alignment::Right),
                    cols[1],
                );
            }
        }
    }
}

fn draw_outline(frame: &mut Frame, area: Rect, app: &mut App) {
    let popup = centered_rect(60, 70, area);
    let items: Vec<ListItem> = app
        .doc
        .headings
        .iter()
        .map(|h| {
            let indent = "  ".repeat(h.depth.saturating_sub(1) as usize);
            ListItem::new(format!("{indent}{}", h.title))
        })
        .collect();
    let title = format!(" Outline ({}) ", app.doc.headings.len());
    let list = List::new(items)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Blue))
                .title(title)
                .title_style(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    frame.render_widget(Clear, popup);
    frame.render_stateful_widget(list, popup, &mut app.outline_state);
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
    config: &RenderConfig,
    watch_rx: Option<&Receiver<()>>,
) -> io::Result<()> {
    loop {
        let (_, term_height) = crossterm_terminal::size()?;
        // Title bar (1 row) + footer (1 row) surround the scrollable body.
        let content_height = term_height.saturating_sub(2).max(1) as usize;

        if let Some(rx) = watch_rx
            && rx.try_iter().last().is_some()
        {
            std::thread::sleep(Duration::from_millis(80));
            while rx.try_recv().is_ok() {}
            reload(app, config, content_height);
        }

        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && handle_key(app, key, content_height)
        {
            return Ok(());
        }
    }
}

fn reload(app: &mut App, config: &RenderConfig, content_height: usize) {
    let Some(path) = app.path.clone() else {
        return;
    };
    match std::fs::read_to_string(&path).map(|content| Document::load(&content, config)) {
        Ok(Ok(doc)) => {
            app.doc = doc;
            app.matches = find_matches(&app.doc.plain_lines, &app.search_input);
            app.current_match = None;
            app.scroll = app.scroll.min(max_scroll(&app.doc, content_height));
            app.status = Some(Status::success("Reloaded"));
        }
        Ok(Err(e)) | Err(e) => {
            app.status = Some(Status::warn(format!("Reload failed: {e}")));
        }
    }
}

/// Run the interactive pager over `content`. When `path` is given, the file
/// is watched and the view auto-reloads on changes; without a path (e.g.
/// piped stdin) the document is static.
pub fn run_pager(content: &str, path: Option<PathBuf>, config: &RenderConfig) -> io::Result<()> {
    let doc = Document::load(content, config)?;

    // Make sure a panic mid-render doesn't leave the user's terminal stuck
    // in raw mode / the alternate screen.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let watcher = path.as_deref().and_then(|p| spawn_watcher(p).ok());
    let title = path
        .as_deref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "stdin".to_string());

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        doc,
        scroll: 0,
        show_outline: false,
        outline_state: ListState::default(),
        mode: Mode::Normal,
        search_input: String::new(),
        matches: Vec::new(),
        current_match: None,
        status: None,
        path,
        title,
    };

    let result = event_loop(
        &mut terminal,
        &mut app,
        config,
        watcher.as_ref().map(|(_, rx)| rx),
    );

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    // Drop back to the default hook now that the terminal is restored.
    let _ = std::panic::take_hook();

    result
}
