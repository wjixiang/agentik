use std::borrow::Cow;
use std::sync::LazyLock;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{StatefulWidget, Widget},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};
use tui_markdown::{Options, StyleSheet, from_str_with_options};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::state::ChatLine;

/// Dark-theme stylesheet for markdown rendering in the chat widget.
#[derive(Debug, Clone, Copy, Default)]
struct PhloemStyleSheet;

impl StyleSheet for PhloemStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::default()
                .fg(Color::Rgb(220, 220, 255))
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            2 => Style::default()
                .fg(Color::Rgb(180, 180, 255))
                .add_modifier(Modifier::BOLD),
            3 => Style::default()
                .fg(Color::Rgb(160, 160, 240))
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Rgb(140, 140, 220))
                .add_modifier(Modifier::ITALIC),
        }
    }

    fn code(&self) -> Style {
        Style::default()
            .fg(Color::Rgb(200, 200, 200))
            .bg(Color::Rgb(40, 40, 40))
    }

    fn link(&self) -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::default()
            .fg(Color::Rgb(180, 180, 100))
            .add_modifier(Modifier::ITALIC)
    }

    fn heading_meta(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn metadata_block(&self) -> Style {
        Style::default().fg(Color::Rgb(180, 180, 160))
    }
}

static MD_OPTIONS: LazyLock<Options<PhloemStyleSheet>> =
    LazyLock::new(|| Options::new(PhloemStyleSheet));

/// State for [`ChatWidget`].
pub struct ChatWidgetState {
    pub total_lines: usize,
    pub viewport_height: u16,
    pub scroll_offset: usize,
    /// Lines rendered during a fresh (non-cached) render, available for caching by the caller.
    pub rendered_lines: Option<Vec<Line<'static>>>,
}

impl ChatWidgetState {
    pub fn new(scroll_offset: usize) -> Self {
        Self {
            total_lines: 0,
            viewport_height: 0,
            scroll_offset,
            rendered_lines: None,
        }
    }
}

/// Chat message list with scrolling and scrollbar support.
pub struct ChatWidget<'a> {
    pub messages: &'a [ChatLine],
    /// Pre-rendered lines from a previous frame (same messages, same width).
    /// When `Some`, the expensive markdown-parse / layout pass is skipped.
    pub cached_lines: Option<Vec<Line<'static>>>,
}

impl StatefulWidget for ChatWidget<'_> {
    type State = ChatWidgetState;

    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        state.viewport_height = area.height;

        let needs_render = self.cached_lines.is_none();

        let lines: Vec<Line<'static>> = if let Some(cached) = self.cached_lines {
            cached
        } else {
            self.messages
                .iter()
                .flat_map(|f| render_line_owned(f, area))
                .collect()
        };

        // If we just freshly rendered, store back for next frame caching.
        if needs_render {
            state.rendered_lines = Some(lines.clone());
        }

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

        state.total_lines = paragraph.line_count(area.width);

        let paragraph = paragraph.scroll((state.scroll_offset as u16, 0));
        paragraph.render(area, buf);

        // Render scrollbar overlaid on the right edge of the chat area
        if state.total_lines > area.height as usize {
            let mut scrollbar_state = ScrollbarState::new(state.total_lines - area.height as usize)
                .position(state.scroll_offset)
                .viewport_content_length(area.height as usize);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::DarkGray))
                .track_style(Style::default().fg(Color::Rgb(40, 40, 40)));
            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}

/// Cell alignment, derived from a markdown table separator (`:--`, `:--:`, `--:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Decode a single separator cell (e.g. `:---:`) into an [`Align`].
fn parse_align(cell: &str) -> Align {
    let t = cell.trim();
    let left = t.starts_with(':');
    let right = t.ends_with(':');
    match (left, right) {
        (true, true) => Align::Center,
        (false, true) => Align::Right,
        _ => Align::Left,
    }
}

/// Strip the leading and trailing `|` from a pipe-delimited table line,
/// returning the inner content.
///
/// Returns `None` unless `s` is at least two characters and both starts and
/// ends with `|`. This guards the slice `s[1..len-1]` against a lone `|`
/// (length 1), which would otherwise panic with an invalid range
/// (`[1..0]`).
fn pipe_inner<'a>(s: &'a str) -> Option<&'a str> {
    if s.len() < 2 || !s.starts_with('|') || !s.ends_with('|') {
        return None;
    }
    Some(&s[1..s.len() - 1])
}

/// Check if a line is a markdown table separator (e.g. `| --- | --- |`).
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    match pipe_inner(trimmed) {
        Some(inner) => inner
            .chars()
            .all(|c| c == '-' || c == '|' || c == ':' || c == ' '),
        None => false,
    }
}

/// Greedily word-wrap a string to fit within `width` display columns.
///
/// Words are split on ASCII spaces; a word longer than the column is
/// hard-broken by character. Each `\n` in the input starts a new output line.
/// Width is measured in terminal display columns (via `unicode-width`), so
/// CJK characters (2 columns wide) and emoji are accounted for correctly.
fn wrap_text(s: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut out: Vec<String> = Vec::new();

    // Append `word` to the current line, hard-breaking when needed.
    let place_word =
        |out: &mut Vec<String>, line: &mut String, line_w: &mut usize, word: &str| {
            // If the word is empty (consecutive spaces) just consume it.
            if word.is_empty() {
                return;
            }
            let word_w = UnicodeWidthStr::width(word);
            if *line_w == 0 {
                // First word on the line.
                if word_w <= width {
                    line.push_str(word);
                    *line_w = word_w;
                } else {
                    push_hard_broken(out, line, line_w, word, width);
                }
            } else if *line_w + 1 + word_w <= width {
                line.push(' ');
                line.push_str(word);
                *line_w += 1 + word_w;
            } else {
                // Doesn't fit — flush current line and start fresh.
                out.push(std::mem::take(line));
                *line_w = 0;
                if word_w <= width {
                    line.push_str(word);
                    *line_w = word_w;
                } else {
                    push_hard_broken(out, line, line_w, word, width);
                }
            }
        };

    for paragraph in s.split('\n') {
        let mut line = String::new();
        let mut line_w = 0usize;
        for word in paragraph.split(' ') {
            place_word(&mut out, &mut line, &mut line_w, word);
        }
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Push a word that is wider than `width` into `line`/`out`, breaking it by
/// character. A single character wider than the column is still emitted (it
/// can't be made narrower), so progress is always guaranteed.
fn push_hard_broken(out: &mut Vec<String>, line: &mut String, line_w: &mut usize, word: &str, width: usize) {
    for ch in word.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if *line_w + cw > width && *line_w > 0 {
            out.push(std::mem::take(line));
            *line_w = 0;
        }
        line.push(ch);
        *line_w += cw;
    }
}

/// Pad/align `content` to exactly `width` display columns.
///
/// Assumes `content` already fits within `width` (callers wrap first); if it
/// is wider, it is returned unchanged so the layout never panics on bad math.
fn align_cell(content: &str, width: usize, align: Align) -> String {
    let w = UnicodeWidthStr::width(content);
    if w >= width {
        return content.to_string();
    }
    let pad = width - w;
    match align {
        Align::Left => format!("{content}{}", " ".repeat(pad)),
        Align::Right => format!("{}{content}", " ".repeat(pad)),
        Align::Center => {
            let left = pad / 2;
            let right = pad - left;
            format!("{}{content}{}", " ".repeat(left), " ".repeat(right))
        }
    }
}

/// Shrink a set of intrinsic column widths down to a total of `target`
/// display columns, never going below a floor of 1 per column.
///
/// Mirrors opencode's proportional fit: each column keeps a share of the
/// available budget proportional to its size above the floor, with the
/// remainder distributed by largest fractional part so the column budget
/// sums to exactly `target`.
fn shrink_widths(widths: &[usize], target: usize) -> Vec<usize> {
    let n = widths.len();
    let total: usize = widths.iter().sum();
    if total <= target || n == 0 {
        return widths.to_vec();
    }
    let floor = 1usize;
    let mut result = vec![floor; n];
    // Room left to distribute above the per-column floor.
    let remaining = target.saturating_sub(floor * n);

    let shrinkable: Vec<usize> = widths.iter().map(|w| w.saturating_sub(floor)).collect();
    let total_shrinkable: usize = shrinkable.iter().sum();
    if total_shrinkable == 0 || remaining == 0 {
        return result;
    }

    // Proportional allocation with largest-remainder rounding.
    let exact: Vec<f64> = shrinkable
        .iter()
        .map(|s| (*s as f64) * (remaining as f64) / (total_shrinkable as f64))
        .collect();
    let mut alloc: Vec<usize> = exact.iter().map(|x| x.floor() as usize).collect();
    let mut assigned: usize = alloc.iter().sum();
    while assigned < remaining {
        // Pick the column with the largest fractional remainder that still
        // has budget headroom above the floor.
        let mut best: Option<usize> = None;
        let mut best_frac = f64::MIN;
        for i in 0..n {
            if alloc[i] >= shrinkable[i] {
                continue;
            }
            let frac = exact[i] - alloc[i] as f64;
            if frac > best_frac {
                best_frac = frac;
                best = Some(i);
            }
        }
        match best {
            Some(i) => {
                alloc[i] += 1;
                assigned += 1;
            }
            // No column can take more — stop to avoid an infinite loop.
            None => break,
        }
    }
    for i in 0..n {
        result[i] += alloc[i];
    }
    result
}

/// Render a markdown table as styled `Line`s with rounded box-drawing borders.
///
/// Layout (porting opencode's table model):
/// - Column content width = the widest cell in that column across all rows,
///   measured in terminal display columns (CJK/emoji aware).
/// - If the table would be wider than `available_width`, columns are
///   proportionally shrunk and cell contents are word-wrapped, so a row may
///   span several rendered lines.
/// - Per-column alignment is read from the `:--:` separator row.
fn render_table_lines(table_lines: &[&str], available_width: usize) -> Vec<Line<'static>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut aligns: Vec<Align> = Vec::new();

    for line in table_lines {
        let trimmed = line.trim();
        let Some(inner) = pipe_inner(trimmed) else {
            continue;
        };
        let cells: Vec<String> = inner.split('|').map(|s| s.trim().to_string()).collect();
        if is_table_separator(trimmed) {
            for c in &cells {
                aligns.push(parse_align(c));
            }
            continue;
        }
        rows.push(cells);
    }

    if rows.is_empty() {
        return table_lines
            .iter()
            .map(|&l| Line::from(l.to_string()))
            .collect();
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return table_lines
            .iter()
            .map(|&l| Line::from(l.to_string()))
            .collect();
    }

    // Pad alignment vector to num_cols (default: left).
    aligns.resize(num_cols, Align::Left);

    // Intrinsic content widths per column (display columns), floor of 1.
    let mut content_widths: Vec<usize> = vec![1; num_cols];
    for row in &rows {
        for (j, cell) in row.iter().enumerate().take(num_cols) {
            content_widths[j] = content_widths[j].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }

    // Each column renders as ` content ` (1-space padding each side); there
    // is one more vertical border than there are columns.
    let fixed_overhead = (num_cols + 1) + 2 * num_cols;
    let budget = available_width.saturating_sub(fixed_overhead);
    let content_widths = shrink_widths(&content_widths, budget);

    // Pre-wrap every cell to its (possibly shrunk) column width.
    let wrapped: Vec<Vec<Vec<String>>> = rows
        .iter()
        .map(|r| {
            (0..num_cols)
                .map(|j| {
                    let cell = r.get(j).map(String::as_str).unwrap_or("");
                    wrap_text(cell, content_widths[j])
                })
                .collect::<Vec<_>>()
        })
        .collect();
    // Each row's height = tallest wrapped cell.
    let row_heights: Vec<usize> = wrapped
        .iter()
        .map(|r| r.iter().map(|c| c.len()).max().unwrap_or(1).max(1))
        .collect();

    let bg_color = Color::Rgb(40, 40, 40);
    let border = Style::default().fg(Color::Rgb(100, 100, 120)).bg(bg_color);
    let header_style = Style::default()
        .fg(Color::Rgb(180, 180, 255))
        .bg(bg_color)
        .add_modifier(Modifier::BOLD);
    let cell_style = Style::default()
        .fg(Color::Rgb(200, 200, 200))
        .bg(bg_color);

    let mut result = Vec::new();

    // Horizontal border line, e.g. `╭──────┬──────╮`.
    let make_border = |left: char, mid: char, right: char| -> Line<'static> {
        let mut spans = vec![Span::styled(left.to_string(), border)];
        for (j, w) in content_widths.iter().enumerate() {
            spans.push(Span::styled("─".repeat(w + 2), border));
            if j + 1 < num_cols {
                spans.push(Span::styled(mid.to_string(), border));
            }
        }
        spans.push(Span::styled(right.to_string(), border));
        Line::from(spans)
    };

    result.push(make_border('╭', '┬', '╮'));

    for (ri, row_cells) in wrapped.iter().enumerate() {
        let style = if ri == 0 { header_style } else { cell_style };
        for sub in 0..row_heights[ri] {
            let mut spans = vec![Span::styled("│".to_string(), border)];
            for j in 0..num_cols {
                let text = row_cells[j].get(sub).map(String::as_str).unwrap_or("");
                let aligned = align_cell(text, content_widths[j], aligns[j]);
                spans.push(Span::styled(format!(" {aligned} "), style));
                spans.push(Span::styled("│".to_string(), border));
            }
            result.push(Line::from(spans));
        }
        if ri == 0 {
            result.push(make_border('├', '┼', '┤'));
        }
    }

    result.push(make_border('╰', '┴', '╯'));
    result
}

/// Render assistant message text with markdown support.
/// Splits the text into non-table segments (rendered via `tui-markdown`)
/// and table segments (rendered with box-drawing characters).
///
/// Non-table segments borrow from `text` directly (no local String allocation),
/// so lifetimes flow naturally. Table segments produce owned `Line<'static>`.
fn render_assistant_text<'a>(text: &'a str, available_width: usize, out: &mut Vec<Line<'a>>) {
    let text_lines: Vec<&str> = text.lines().collect();
    if text_lines.is_empty() {
        return;
    }

    let mut pos = 0;
    while pos < text_lines.len() {
        let trimmed = text_lines[pos].trim();
        let is_table = trimmed.starts_with('|')
            && trimmed.ends_with('|')
            && pos + 1 < text_lines.len()
            && is_table_separator(text_lines[pos + 1].trim());

        if is_table {
            let start = pos;
            while pos < text_lines.len() && text_lines[pos].trim().starts_with('|') {
                pos += 1;
            }
            // Table rendering produces owned Lines ('static), coerces to '_
            out.extend(render_table_lines(&text_lines[start..pos], available_width));
        } else {
            let start = pos;
            while pos < text_lines.len() {
                let t = text_lines[pos].trim();
                let table_ahead = t.starts_with('|')
                    && t.ends_with('|')
                    && pos + 1 < text_lines.len()
                    && is_table_separator(text_lines[pos + 1].trim());
                if table_ahead {
                    break;
                }
                pos += 1;
            }
            if pos == start {
                // No non-table lines consumed — skip to avoid empty segment
                continue;
            }
            // Compute byte range within the original text for this segment.
            let seg_start = text_lines[start].as_ptr() as usize - text.as_ptr() as usize;
            let seg_end = text_lines[pos - 1].as_ptr() as usize + text_lines[pos - 1].len()
                - text.as_ptr() as usize;
            let segment = &text[seg_start..seg_end];
            let md_text = from_str_with_options(segment, &MD_OPTIONS);
            out.extend(md_text.lines);
        }
    }
}

/// Convert borrowed `Line` / `Span` data into fully owned `Line<'static>`.
fn into_owned_lines(lines: Vec<Line<'_>>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            Line::from(
                line.spans
                    .into_iter()
                    .map(|span| Span::styled(
                        span.content.into_owned(),
                        span.style,
                    ))
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

/// Render a single `ChatLine` into one or more owned `Line<'static>`.
fn render_line_owned(msg: &ChatLine, area: Rect) -> Vec<Line<'static>> {
    match msg {
        ChatLine::User(text) => {
            let mut lines = vec![Line::from(Span::styled(
                "You",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))];
            for line in text.lines() {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Cyan),
                )));
            }
            lines
        }
        ChatLine::Assistant(text) => {
            let mut lines: Vec<Line<'_>> = vec![Line::from(Span::styled(
                "Assistant",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ))];
            render_assistant_text(text, area.width as usize, &mut lines);
            into_owned_lines(lines)
        }
        ChatLine::Thinking(text) => {
            let mut lines = Vec::new();
            let mut first = true;
            for line in text.lines() {
                let prefix = if first { "💭 " } else { "   " };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{line}"),
                    Style::default().fg(Color::DarkGray),
                )));
                first = false;
            }
            lines
        }
        ChatLine::ToolCall { name, input } => {
            let mut lines = vec![Line::from(Span::styled(
                format!("🔧 Calling: {}", name),
                Style::default().fg(Color::Yellow),
            ))];
            if !input.is_empty() {
                // Render each input parameter on its own indented line.
                for line in input.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("   {}", line),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
            lines
        }
        ChatLine::ToolBackground { id: _id, name } => {
            vec![Line::from(Span::styled(
                format!("⏳ Calling: {} (running in background)", name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM),
            ))]
        }
        ChatLine::ToolResult { ok, content } => {
            let icon = if *ok { "✓" } else { "✗" };
            let color = if *ok { Color::Green } else { Color::Red };
            let mut lines = Vec::new();
            let mut first = true;
            for line in content.lines() {
                let prefix = if first {
                    format!("{icon} ")
                } else {
                    "  ".to_string()
                };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{line}"),
                    Style::default().fg(color),
                )));
                first = false;
            }
            lines
        }
        ChatLine::Error(text) => {
            let mut lines = Vec::new();
            let mut first = true;
            for line in text.lines() {
                let prefix = if first { "✗ Error: " } else { "         " };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{line}"),
                    Style::default().fg(Color::Red),
                )));
                first = false;
            }
            lines
        }
        ChatLine::Separator => {
            vec![Line::from(Span::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(Color::DarkGray),
            ))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_inner_handles_lone_pipe_without_panicking() {
        // Regression: a bare `|` (length 1) previously made
        // `trimmed[1..trimmed.len() - 1]` slice `[1..0]` and panic.
        assert_eq!(pipe_inner("|"), None);
        assert_eq!(is_table_separator("|"), false);

        // Normal cases.
        assert_eq!(pipe_inner("| a | b |"), Some(" a | b "));
        assert!(is_table_separator("| --- | --- |"));
        assert!(!is_table_separator("| a | b |"));

        // Empty-content pipe pair is still a valid inner slice.
        assert_eq!(pipe_inner("||"), Some(""));
    }

    #[test]
    fn parse_align_reads_separator_hints() {
        assert_eq!(parse_align("---"), Align::Left);
        assert_eq!(parse_align(":---"), Align::Left);
        assert_eq!(parse_align("---:"), Align::Right);
        assert_eq!(parse_align(":---:"), Align::Center);
        // Whitespace is tolerated.
        assert_eq!(parse_align("  :--:  "), Align::Center);
    }

    #[test]
    fn wrap_text_breaks_to_display_width() {
        // ASCII word wrap.
        assert_eq!(wrap_text("hello world", 5), vec!["hello", "world"]);
        // A word longer than the column is hard-broken by character.
        assert_eq!(wrap_text("abcdef", 3), vec!["abc", "def"]);
        // CJK characters take 2 columns each — two fit in a width-4 column.
        assert_eq!(wrap_text("你好世界", 4), vec!["你好", "世界"]);
        // Multiple spaces collapse: `split(' ')` yields an empty word that is
        // dropped, so the run is treated as a single separator.
        assert_eq!(wrap_text("a  b", 10), vec!["a b"]);
        // Empty input still yields one (empty) line.
        assert_eq!(wrap_text("", 5), vec![""]);
    }

    #[test]
    fn shrink_widths_fits_to_target() {
        // No shrink needed when under target.
        assert_eq!(shrink_widths(&[3, 5], 20), vec![3, 5]);
        // Proportional shrink keeps the floor and sums to target.
        let shrunken = shrink_widths(&[10, 30], 12);
        assert_eq!(shrunken.iter().sum::<usize>(), 12);
        assert!(shrunken.iter().all(|&w| w >= 1));
        // The wider column keeps more of the budget than the narrow one.
        assert!(shrunken[1] > shrunken[0]);
        // Never goes below the floor of 1, even with a tiny target.
        let tiny = shrink_widths(&[5, 5, 5], 0);
        assert_eq!(tiny, vec![1, 1, 1]);
    }

    #[test]
    fn render_table_lines_respects_width_and_aligns() {
        let src = "| name | age |\n| :--- | ---: |\n| Alice | 30 |\n| Bob | 5 |\n";
        let lines: Vec<&str> = src.lines().collect();
        // Wide viewport: table fits, no wrapping (4 logical rows + 2 borders).
        let rendered = render_table_lines(&lines, 80);
        assert!(rendered.len() >= 6); // top, header, sep, 2 rows, bottom
        // Top border uses rounded corners.
        assert!(rendered[0].spans.iter().any(|s| s.content.contains('╭')));

        // Narrow viewport forces wrapping — header + 2 data rows may each
        // expand to multiple lines, so the output must be at least as tall.
        let narrow = render_table_lines(&lines, 12);
        assert!(narrow.len() >= rendered.len());
    }
}
