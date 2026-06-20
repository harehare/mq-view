//! Best-effort ASCII-art rendering of simple mermaid flowcharts.
//!
//! This only understands a small subset of mermaid `graph`/`flowchart` syntax
//! (nodes, shapes, and edges with optional labels). Other diagram types
//! (sequenceDiagram, classDiagram, gantt, ...) and advanced flowchart syntax
//! (subgraph styling, click handlers, complex multi-label edges, ...) are not
//! laid out; callers should fall back to rendering the raw source when `render`
//! returns `None`.
use crate::renderer::visible_width;
use colored::*;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Direction {
    #[default]
    TopDown,
    LeftRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Rectangle,
    Rounded,
    Diamond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeStyle {
    Solid,
    Dotted,
    Thick,
}

#[derive(Debug, Clone)]
struct MNode {
    label: String,
    shape: Shape,
}

#[derive(Debug, Clone)]
struct MEdge {
    from: String,
    to: String,
    label: Option<String>,
    style: EdgeStyle,
}

#[derive(Debug, Default)]
struct MermaidGraph {
    direction: Direction,
    order: Vec<String>,
    nodes: HashMap<String, MNode>,
    edges: Vec<MEdge>,
}

const ARROWS: &[(&str, EdgeStyle)] = &[
    ("-.->", EdgeStyle::Dotted),
    ("-.-", EdgeStyle::Dotted),
    ("==>", EdgeStyle::Thick),
    ("===", EdgeStyle::Thick),
    ("-->", EdgeStyle::Solid),
    ("---", EdgeStyle::Solid),
    ("--x", EdgeStyle::Solid),
    ("--o", EdgeStyle::Solid),
];

/// Render mermaid source as ASCII art, returning `None` for unsupported
/// diagram types or syntax this module can't make sense of.
pub fn render(source: &str, max_width: usize) -> Option<String> {
    let graph = parse(source)?;
    if graph.order.is_empty() {
        return None;
    }
    Some(layout(&graph, max_width))
}

fn parse(source: &str) -> Option<MermaidGraph> {
    let mut lines = source.lines().map(str::trim).filter(|l| !l.is_empty());

    let header = lines.next()?;
    let mut header_parts = header.split_whitespace();
    let kind = header_parts.next()?.to_lowercase();
    if kind != "graph" && kind != "flowchart" {
        return None;
    }
    let direction = match header_parts.next().map(str::to_uppercase).as_deref() {
        Some("LR") | Some("RL") => Direction::LeftRight,
        _ => Direction::TopDown,
    };

    let mut graph = MermaidGraph {
        direction,
        ..Default::default()
    };

    for line in lines {
        if line.starts_with("%%")
            || line == "end"
            || line.starts_with("subgraph")
            || line.starts_with("classDef")
            || line.starts_with("class ")
            || line.starts_with("style ")
            || line.starts_with("click ")
            || line.starts_with("linkStyle")
        {
            continue;
        }
        parse_line(line, &mut graph);
    }

    Some(graph)
}

fn parse_line(line: &str, graph: &mut MermaidGraph) {
    let mut rest = line;
    let mut pending_from: Option<String> = None;

    loop {
        let found = ARROWS
            .iter()
            .filter_map(|(pat, style)| rest.find(pat).map(|pos| (pos, *pat, *style)))
            .min_by_key(|(pos, pat, _)| (*pos, std::cmp::Reverse(pat.len())));

        let Some((pos, pat, style)) = found else {
            let text = rest.trim();
            if !text.is_empty() {
                let id = register_node(graph, text);
                if let Some(from) = pending_from.take() {
                    graph.edges.push(MEdge {
                        from,
                        to: id,
                        label: None,
                        style: EdgeStyle::Solid,
                    });
                }
            }
            break;
        };

        let before = rest[..pos].trim();
        let from_id = if let Some(from) = pending_from.take() {
            from
        } else {
            register_node(graph, before)
        };

        let mut after = &rest[pos + pat.len()..];
        let mut label = None;
        let after_trimmed = after.trim_start();
        if after_trimmed.starts_with('|')
            && let Some(end) = after_trimmed[1..].find('|')
        {
            label = Some(after_trimmed[1..1 + end].trim().to_string());
            after = &after_trimmed[1 + end + 1..];
        }

        let next_arrow_pos = ARROWS.iter().filter_map(|(p, _)| after.find(p)).min();

        let node_text = match next_arrow_pos {
            Some(p) => after[..p].trim(),
            None => after.trim(),
        };
        let to_id = register_node(graph, node_text);

        graph.edges.push(MEdge {
            from: from_id,
            to: to_id.clone(),
            label,
            style,
        });

        if next_arrow_pos.is_some() {
            pending_from = Some(to_id);
            rest = after;
        } else {
            break;
        }
    }
}

/// Parse a single node token like `A`, `A[Label]`, `A(Label)`, `A((Label))`,
/// `A{Label}`, registering (or updating) it in the graph and returning its id.
fn register_node(graph: &mut MermaidGraph, text: &str) -> String {
    let text = text.trim();
    let id_len = text
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .count();
    let id: String = text.chars().take(id_len).collect();
    let id = if id.is_empty() { text.to_string() } else { id };
    let remainder = text[id_len.min(text.len())..].trim();

    let (shape, label) = if remainder.is_empty() {
        (Shape::Rectangle, None)
    } else if let Some(inner) = strip_pair(remainder, "((", "))") {
        (Shape::Rounded, Some(inner))
    } else if let Some(inner) = strip_pair(remainder, "([", "])") {
        (Shape::Rounded, Some(inner))
    } else if let Some(inner) = strip_pair(remainder, "(", ")") {
        (Shape::Rounded, Some(inner))
    } else if let Some(inner) = strip_pair(remainder, "{", "}") {
        (Shape::Diamond, Some(inner))
    } else if let Some(inner) = strip_pair(remainder, "[", "]") {
        (Shape::Rectangle, Some(inner))
    } else if let Some(inner) = strip_pair(remainder, ">", "]") {
        (Shape::Rectangle, Some(inner))
    } else {
        (Shape::Rectangle, Some(remainder.to_string()))
    };

    let label = label
        .map(|l| l.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| id.clone());

    if !graph.order.iter().any(|existing| existing == &id) {
        graph.order.push(id.clone());
    }
    graph
        .nodes
        .entry(id.clone())
        .and_modify(|n| {
            // Prefer a richer label/shape over a bare id-only reference.
            if label != id {
                n.label = label.clone();
                n.shape = shape;
            }
        })
        .or_insert(MNode { label, shape });

    id
}

fn strip_pair(text: &str, open: &str, close: &str) -> Option<String> {
    if text.starts_with(open) && text.ends_with(close) && text.len() >= open.len() + close.len() {
        Some(text[open.len()..text.len() - close.len()].to_string())
    } else {
        None
    }
}

fn box_chars(shape: Shape) -> (char, char, char, char, char, char) {
    match shape {
        Shape::Rectangle => ('┌', '┐', '└', '┘', '─', '│'),
        Shape::Rounded => ('╭', '╮', '╰', '╯', '─', '│'),
        Shape::Diamond => ('╔', '╗', '╚', '╝', '═', '║'),
    }
}

fn shape_color(s: &str, shape: Shape) -> ColoredString {
    match shape {
        Shape::Rectangle => s.cyan(),
        Shape::Rounded => s.green(),
        Shape::Diamond => s.yellow(),
    }
}

/// Render a single node as a 3-line box, returning the lines and their
/// (unstyled) display width.
fn render_box(node: &MNode) -> (Vec<String>, usize) {
    let (tl, tr, bl, br, h, v) = box_chars(node.shape);
    let width = visible_width(&node.label) + 2;
    let top = format!("{}{}{}", tl, h.to_string().repeat(width), tr);
    let bottom = format!("{}{}{}", bl, h.to_string().repeat(width), br);
    let total_width = width + 2;
    let vbar = shape_color(&v.to_string(), node.shape).to_string();
    (
        vec![
            shape_color(&top, node.shape).to_string(),
            format!("{} {} {}", vbar, node.label, vbar),
            shape_color(&bottom, node.shape).to_string(),
        ],
        total_width,
    )
}

fn arrow_label(label: &Option<String>) -> String {
    label.clone().unwrap_or_default()
}

fn edge_style_arrow(style: EdgeStyle, horizontal: bool) -> &'static str {
    match (style, horizontal) {
        (EdgeStyle::Solid, true) => "──▶",
        (EdgeStyle::Dotted, true) => "┄┄▶",
        (EdgeStyle::Thick, true) => "══▶",
        (EdgeStyle::Solid, false) => "│",
        (EdgeStyle::Dotted, false) => "┊",
        (EdgeStyle::Thick, false) => "║",
    }
}

fn rank_of(graph: &MermaidGraph) -> HashMap<String, usize> {
    let mut indegree: HashMap<&str, usize> = graph.order.iter().map(|n| (n.as_str(), 0)).collect();
    for edge in &graph.edges {
        if let Some(d) = indegree.get_mut(edge.to.as_str()) {
            *d += 1;
        }
    }

    let mut remaining = indegree.clone();
    let mut rank: HashMap<String, usize> = HashMap::new();
    // Tracks nodes that have been assigned a final rank and enqueued exactly
    // once. Edges pointing back into a `settled` node are cycle back-edges;
    // they're ignored for layering so Kahn's algorithm can never stall or
    // re-enqueue the same node forever.
    let mut settled: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    for id in &graph.order {
        if indegree[id.as_str()] == 0 {
            rank.insert(id.clone(), 0);
            settled.insert(id.clone());
            queue.push_back(id.clone());
        }
    }

    loop {
        while let Some(id) = queue.pop_front() {
            let r = rank[&id];
            for edge in graph.edges.iter().filter(|e| e.from == id) {
                if settled.contains(&edge.to) {
                    continue;
                }
                let entry = rank.entry(edge.to.clone()).or_insert(0);
                if r + 1 > *entry {
                    *entry = r + 1;
                }
                if let Some(d) = remaining.get_mut(edge.to.as_str()) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        settled.insert(edge.to.clone());
                        queue.push_back(edge.to.clone());
                    }
                }
            }
        }

        // Anything left unsettled is part of a cycle: every remaining node
        // still has an unsettled predecessor. Break the deadlock by ranking
        // the next such node from whichever predecessors are already
        // settled (ignoring the back-edge), then resume.
        let next = graph.order.iter().find(|id| !settled.contains(id.as_str()));
        let Some(next) = next else { break };

        let predecessor_rank = graph
            .edges
            .iter()
            .filter(|e| &e.to == next && settled.contains(&e.from))
            .filter_map(|e| rank.get(&e.from))
            .max()
            .copied();
        let r = predecessor_rank.map(|p| p + 1).unwrap_or(0);
        rank.insert(next.clone(), r);
        settled.insert(next.clone());
        queue.push_back(next.clone());
    }

    rank
}

fn simple_chain(graph: &MermaidGraph) -> Option<Vec<String>> {
    if graph.order.is_empty() || graph.edges.len() != graph.order.len().saturating_sub(1) {
        return None;
    }
    let mut outdeg: HashMap<&str, usize> = HashMap::new();
    let mut indeg: HashMap<&str, usize> = HashMap::new();
    for edge in &graph.edges {
        *outdeg.entry(edge.from.as_str()).or_insert(0) += 1;
        *indeg.entry(edge.to.as_str()).or_insert(0) += 1;
    }
    if outdeg.values().any(|&d| d > 1) || indeg.values().any(|&d| d > 1) {
        return None;
    }

    let start = graph
        .order
        .iter()
        .find(|id| indeg.get(id.as_str()).copied().unwrap_or(0) == 0)?;

    let mut chain = vec![start.clone()];
    let mut current = start.clone();
    let next_of: HashMap<&str, &str> = graph
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    while let Some(next) = next_of.get(current.as_str()) {
        chain.push(next.to_string());
        current = next.to_string();
        if chain.len() > graph.order.len() {
            return None; // cycle guard
        }
    }

    if chain.len() == graph.order.len() {
        Some(chain)
    } else {
        None
    }
}

fn edge_between<'a>(graph: &'a MermaidGraph, from: &str, to: &str) -> Option<&'a MEdge> {
    graph.edges.iter().find(|e| e.from == from && e.to == to)
}

fn layout(graph: &MermaidGraph, max_width: usize) -> String {
    let mut out = String::new();

    if graph.direction == Direction::LeftRight
        && let Some(chain) = simple_chain(graph)
    {
        render_horizontal_chain(graph, &chain, max_width, &mut out);
        return out;
    }

    let rank = rank_of(graph);
    let max_rank = rank.values().copied().max().unwrap_or(0);
    let mut ranks: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];
    for id in &graph.order {
        ranks[rank[id]].push(id.clone());
    }

    let mut rendered_edges = vec![false; graph.edges.len()];

    for (r, row) in ranks.iter().enumerate() {
        render_row(graph, row, &mut out);

        if r + 1 >= ranks.len() {
            continue;
        }
        let next = &ranks[r + 1];
        let edge_indices: Vec<usize> = graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| row.contains(&e.from) && next.contains(&e.to))
            .map(|(i, _)| i)
            .collect();
        for &i in &edge_indices {
            rendered_edges[i] = true;
        }

        if row.len() == 1 && next.len() == 1 && edge_indices.len() == 1 {
            let edge = &graph.edges[edge_indices[0]];
            let label = arrow_label(&edge.label);
            let width = box_display_width(&graph.nodes[&row[0]]);
            let pad = width / 2;
            out.push_str(&" ".repeat(pad));
            out.push_str(edge_style_arrow(edge.style, false));
            out.push('\n');
            if !label.is_empty() {
                out.push_str(&" ".repeat(pad.saturating_sub(visible_width(&label) / 2)));
                out.push_str(&label.italic().to_string());
                out.push('\n');
            }
            out.push_str(&" ".repeat(pad));
            out.push('▼');
            out.push('\n');
        } else if !edge_indices.is_empty() {
            for &i in &edge_indices {
                render_edge_line(&mut out, graph, &graph.edges[i]);
            }
            out.push('\n');
        }
    }

    let leftover: Vec<usize> = (0..graph.edges.len())
        .filter(|&i| !rendered_edges[i])
        .collect();
    if !leftover.is_empty() {
        out.push_str(&"(other connections)\n".bright_black().to_string());
        for i in leftover {
            render_edge_line(&mut out, graph, &graph.edges[i]);
        }
    }

    out
}

fn render_edge_line(out: &mut String, graph: &MermaidGraph, edge: &MEdge) {
    let from_label = &graph.nodes[&edge.from].label;
    let to_label = &graph.nodes[&edge.to].label;
    let arrow = edge_style_arrow(edge.style, true);
    let label = arrow_label(&edge.label);
    if label.is_empty() {
        out.push_str(&format!(
            "  {} {} {}\n",
            from_label.bright_black(),
            arrow.bright_black(),
            to_label.bright_black()
        ));
    } else {
        out.push_str(&format!(
            "  {} {}[{}]{} {}\n",
            from_label.bright_black(),
            arrow.bright_black(),
            label.italic(),
            arrow.bright_black(),
            to_label.bright_black()
        ));
    }
}

fn box_display_width(node: &MNode) -> usize {
    visible_width(&node.label) + 4
}

fn render_row(graph: &MermaidGraph, row: &[String], out: &mut String) {
    if row.is_empty() {
        return;
    }
    let boxes: Vec<Vec<String>> = row
        .iter()
        .map(|id| render_box(&graph.nodes[id]).0)
        .collect();

    for line_idx in 0..3 {
        let mut line = String::new();
        for (i, b) in boxes.iter().enumerate() {
            if i > 0 {
                line.push_str("   ");
            }
            line.push_str(&b[line_idx]);
        }
        out.push_str(&line);
        out.push('\n');
    }
}

fn render_horizontal_chain(
    graph: &MermaidGraph,
    chain: &[String],
    max_width: usize,
    out: &mut String,
) {
    let mut lines = vec![String::new(), String::new(), String::new()];
    let mut current_width = 0usize;

    for (i, id) in chain.iter().enumerate() {
        let (b, w) = render_box(&graph.nodes[id]);

        let mut segment_width = w;
        let mut connector: Option<(String, usize)> = None;
        if i > 0 {
            let prev = &chain[i - 1];
            if let Some(edge) = edge_between(graph, prev, id) {
                let arrow = edge_style_arrow(edge.style, true);
                let label = arrow_label(&edge.label);
                let text = if label.is_empty() {
                    format!(" {} ", arrow)
                } else {
                    format!(" {}[{}] ", arrow, label)
                };
                segment_width += visible_width(&text);
                connector = Some((text, w));
            }
        }

        if current_width > 0 && current_width + segment_width > max_width {
            for l in lines.iter() {
                out.push_str(l);
                out.push('\n');
            }
            out.push('\n');
            lines = vec![String::new(), String::new(), String::new()];
            current_width = 0;
        }

        if let Some((text, _)) = connector {
            let text_width = visible_width(&text);
            lines[1].push_str(&text);
            lines[0].push_str(&" ".repeat(text_width));
            lines[2].push_str(&" ".repeat(text_width));
            current_width += text_width;
        }

        for (idx, l) in b.iter().enumerate() {
            lines[idx].push_str(l);
        }
        current_width += w;
    }

    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_simple_chain_td() {
        let result = render("graph TD\nA-->B-->C", 80).unwrap();
        assert!(result.contains('A'));
        assert!(result.contains('B'));
        assert!(result.contains('C'));
        assert!(result.contains('▼'));
    }

    #[test]
    fn test_render_simple_chain_lr() {
        let result = render("graph LR\nA-->B-->C", 80).unwrap();
        assert!(result.contains('A'));
        assert!(result.contains("──▶"));
    }

    #[test]
    fn test_render_labeled_edge() {
        let result = render("graph TD\nA-->|yes|B", 80).unwrap();
        assert!(result.contains("yes"));
    }

    #[test]
    fn test_render_node_shapes_and_labels() {
        let result = render("graph TD\nA[Start]-->B{Decision}-->C(End)", 80).unwrap();
        assert!(result.contains("Start"));
        assert!(result.contains("Decision"));
        assert!(result.contains("End"));
    }

    #[test]
    fn test_render_branching_falls_back_to_list() {
        let result = render("graph TD\nA-->B\nA-->C", 80).unwrap();
        assert!(result.contains('B'));
        assert!(result.contains('C'));
    }

    #[test]
    fn test_render_graph_with_cycle_terminates_and_keeps_all_edges() {
        // A regression test for a deadlock in the rank-layering algorithm:
        // the back-edge D-->B previously caused Kahn's algorithm to stall
        // forever once a node was re-queued infinitely.
        let result = render("graph TD\nA-->B\nB-->|Yes|C\nB-->|No|D\nD-->B\nC-->E", 80).unwrap();
        for label in ["A", "B", "C", "D", "E"] {
            assert!(result.contains(label), "missing node {label}");
        }
    }

    #[test]
    fn test_unsupported_diagram_returns_none() {
        assert!(render("sequenceDiagram\nAlice->>Bob: Hello", 80).is_none());
    }

    #[test]
    fn test_empty_returns_none() {
        assert!(render("", 80).is_none());
    }
}
