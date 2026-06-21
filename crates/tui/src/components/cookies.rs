use crate::tui::{TEXT, TuiApp, apply_selection, count_visual_lines};

pub fn render_response_cookies_tab(
    frame: &mut ratatui::Frame,
    app: &TuiApp,
    area: ratatui::layout::Rect,
    inner: ratatui::layout::Rect,
    view_height: usize,
) {
    let raw_string = app.active_response_string();

    let mut text = ratatui::text::Text::raw(raw_string.clone());
    text = apply_selection(text, app.text_selection);

    let mut p = ratatui::widgets::Paragraph::new(text)
        .style(ratatui::style::Style::default().fg(TEXT))
        .scroll((app.response_scroll, 0));

    if app.wrap_body {
        p = p.wrap(ratatui::widgets::Wrap { trim: false });
    }

    frame.render_widget(p, inner);

    let inner_width = inner.width as usize;
    let lines = if app.wrap_body && inner_width > 0 {
        count_visual_lines(&raw_string, inner_width, app.wrap_body)
    } else {
        raw_string.lines().count()
    };

    if lines > view_height {
        let mut scrollbar_state = ratatui::widgets::ScrollbarState::default()
            .content_length(lines.saturating_sub(view_height))
            .position(app.response_scroll as usize);
        let scrollbar = ratatui::widgets::Scrollbar::default()
            .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
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
