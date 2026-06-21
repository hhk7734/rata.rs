use ratatui::layout::Constraint;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Row, Table};

use crate::tui::{ActiveBlock, ParamEditMode, TuiApp};
use crate::tui::{BLUE, MUTED, SELECTED_BG, TEXT, muted_style, render_with_cursor_spans};

pub fn render_request_headers_tab(
    frame: &mut ratatui::Frame,
    app: &TuiApp,
    area: ratatui::layout::Rect,
    block: Block<'static>,
) {
    let mut rows = Vec::new();
    let params = &app.draft.headers;
    for (i, param) in params.iter().enumerate() {
        let display_key = if app.active_block == ActiveBlock::Params
            && app.param_edit_mode == ParamEditMode::Key
            && i == app.selected_request_row
        {
            Line::from(render_with_cursor_spans(
                &param.key,
                app.text_cursor,
                app.cursor_visible(),
                Style::default(),
            ))
        } else {
            Line::from(param.key.clone())
        };
        let display_value = if app.active_block == ActiveBlock::Params
            && app.param_edit_mode == ParamEditMode::Value
            && i == app.selected_request_row
        {
            Line::from(render_with_cursor_spans(
                &param.value,
                app.text_cursor,
                app.cursor_visible(),
                Style::default(),
            ))
        } else {
            Line::from(param.value.clone())
        };
        let checkbox_text = if param.enabled { "[x]" } else { "[ ]" }.to_string();
        let checkbox_cell = if param.required {
            ratatui::widgets::Cell::from(checkbox_text).style(Style::default().fg(MUTED))
        } else {
            ratatui::widgets::Cell::from(checkbox_text)
        };
        let mut row = Row::new(vec![
            checkbox_cell,
            ratatui::widgets::Cell::from(display_key),
            ratatui::widgets::Cell::from(display_value),
        ]);
        if app.active_block == ActiveBlock::Params && i == app.selected_request_row {
            row = row.style(Style::default().bg(SELECTED_BG));
        }
        rows.push(row);
    }
    let add_style =
        if app.active_block == ActiveBlock::Params && app.selected_request_row == params.len() {
            Style::default().bg(SELECTED_BG)
        } else {
            Style::default()
        };
    rows.push(
        Row::new(vec![
            ratatui::widgets::Cell::from(""),
            ratatui::widgets::Cell::from("<Add new header...>").style(add_style.fg(MUTED)),
            ratatui::widgets::Cell::from(""),
        ])
        .style(add_style),
    );

    let t = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Percentage(30),
            Constraint::Percentage(67),
        ],
    )
    .header(Row::new(["", "Key", "Value"]).style(muted_style().add_modifier(Modifier::BOLD)))
    .block(block)
    .style(Style::default().fg(TEXT));
    frame.render_widget(t, area);
}

pub fn render_response_headers_tab(
    frame: &mut ratatui::Frame,
    app: &TuiApp,
    area: ratatui::layout::Rect,
    inner: ratatui::layout::Rect,
    view_height: usize,
) {
    use ratatui::text::Span;
    let header_rows: Vec<ratatui::widgets::Row> = app
        .response
        .headers
        .iter()
        .map(|(k, v)| {
            ratatui::widgets::Row::new(vec![
                ratatui::widgets::Cell::from(Span::styled(k.clone(), Style::default().fg(BLUE))),
                ratatui::widgets::Cell::from(Span::raw(v.clone())),
            ])
        })
        .collect();
    let widths = [
        ratatui::layout::Constraint::Percentage(30),
        ratatui::layout::Constraint::Percentage(70),
    ];
    let table = ratatui::widgets::Table::new(header_rows, widths)
        .header(
            ratatui::widgets::Row::new(vec!["Key", "Value"])
                .style(Style::default().add_modifier(Modifier::BOLD).fg(MUTED)),
        )
        .column_spacing(2);

    let mut table_state =
        ratatui::widgets::TableState::default().with_offset(app.response_scroll as usize);
    frame.render_stateful_widget(table, inner, &mut table_state);

    let lines = app.response.headers.len() + 1; // +1 for header row
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
