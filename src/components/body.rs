use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::tui::{Selection, TEXT, apply_cursor_to_text, apply_selection, count_visual_lines};

pub fn render_body_with_scrollbar<'a>(
    frame: &mut Frame,
    area: Rect,
    raw_text: &str,
    block: Option<Block<'a>>,
    scroll: u16,
    wrap: bool,
    selection: Option<Selection>,
    cursor: Option<usize>,
) {
    let display_text = pretty_body(raw_text);
    let mut text = highlight_json(&display_text);

    text = apply_selection(text, selection);
    if let Some(c) = cursor {
        text = apply_cursor_to_text(text, c);
    }
    let mut p = Paragraph::new(text)
        .style(Style::default().fg(TEXT))
        .scroll((scroll, 0));

    if let Some(b) = block {
        p = p.block(b);
    }

    if wrap {
        p = p.wrap(Wrap { trim: false });
    }

    frame.render_widget(p, area);

    let inner_width = area.width.saturating_sub(2) as usize;
    let lines = if wrap && inner_width > 0 {
        count_visual_lines(&display_text, inner_width, wrap)
    } else {
        display_text.lines().count()
    };

    let view_height = area.height.saturating_sub(2) as usize;
    if lines > view_height {
        let mut scrollbar_state = ScrollbarState::default()
            .content_length(lines.saturating_sub(view_height))
            .position(scroll as usize);
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));
        frame.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

pub fn pretty_body(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| body.to_string())
}

fn highlight_json(json: &str) -> ratatui::text::Text<'static> {
    use ratatui::style::Color;
    use ratatui::text::{Line, Span};

    let mut lines = Vec::new();
    for line in json.lines() {
        let mut spans = Vec::new();
        let mut current_span = String::new();
        let mut in_string = false;
        let mut escaped = false;

        let mut iter = line.chars().peekable();
        while let Some(c) = iter.next() {
            if in_string {
                current_span.push(c);
                if c == '\\' && !escaped {
                    escaped = true;
                } else if c == '"' && !escaped {
                    in_string = false;
                    let mut is_key_local = false;
                    let mut lookahead = iter.clone();
                    while let Some(lc) = lookahead.next() {
                        if lc == ' ' {
                            continue;
                        }
                        if lc == ':' {
                            is_key_local = true;
                        }
                        break;
                    }
                    if is_key_local {
                        spans.push(Span::styled(
                            current_span.clone(),
                            Style::default().fg(Color::LightBlue),
                        ));
                    } else {
                        spans.push(Span::styled(
                            current_span.clone(),
                            Style::default().fg(Color::Green),
                        ));
                    }
                    current_span.clear();
                } else {
                    escaped = false;
                }
            } else {
                if c == '"' {
                    if !current_span.is_empty() {
                        spans.extend(highlight_non_string(&current_span));
                        current_span.clear();
                    }
                    in_string = true;
                    current_span.push(c);
                } else {
                    current_span.push(c);
                }
            }
        }
        if !current_span.is_empty() {
            if in_string {
                spans.push(Span::styled(
                    current_span,
                    Style::default().fg(Color::Green),
                ));
            } else {
                spans.extend(highlight_non_string(&current_span));
            }
        }
        lines.push(Line::from(spans));
    }
    ratatui::text::Text::from(lines)
}

fn highlight_non_string(text: &str) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::text::Span;
    let mut spans = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if " \t\r\n{}[],:".contains(c) {
            if !current.is_empty() {
                spans.push(highlight_value(&current));
                current.clear();
            }
            spans.push(Span::raw(c.to_string()));
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        spans.push(highlight_value(&current));
    }
    spans
}

fn highlight_value(text: &str) -> ratatui::text::Span<'static> {
    use ratatui::style::Color;
    use ratatui::text::Span;
    match text {
        "true" | "false" => Span::styled(text.to_string(), Style::default().fg(Color::Yellow)),
        "null" => Span::styled(text.to_string(), Style::default().fg(Color::DarkGray)),
        _ if text.chars().all(|c| {
            c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+'
        }) =>
        {
            Span::styled(text.to_string(), Style::default().fg(Color::Magenta))
        }
        _ => Span::raw(text.to_string()),
    }
}
