use ratatui::layout::Constraint;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Row, Table};

use crate::tui::{ActiveBlock, ParamEditMode, TuiApp};
use crate::tui::{MUTED, SELECTED_BG, TEXT, muted_style, render_with_cursor_spans};

pub fn render_query_tab(
    frame: &mut ratatui::Frame,
    app: &TuiApp,
    area: ratatui::layout::Rect,
    block: Block<'static>,
) {
    let mut rows = Vec::new();
    let params = &app.draft.params;
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
            ratatui::widgets::Cell::from(""),
        ]);
        if app.active_block == ActiveBlock::Params && i == app.selected_request_row {
            row = row.style(Style::default().bg(SELECTED_BG));
        }
        rows.push(row);
    }
    let add_style = if app.active_block == ActiveBlock::Params
        && app.selected_request_row == params.len()
    {
        Style::default().bg(SELECTED_BG)
    } else {
        Style::default()
    };
    rows.push(
        Row::new(vec![
            ratatui::widgets::Cell::from(""),
            ratatui::widgets::Cell::from("<Add new query...>").style(add_style.fg(MUTED)),
            ratatui::widgets::Cell::from(""),
            ratatui::widgets::Cell::from(""),
        ])
        .style(add_style),
    );

    let t = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Percentage(25),
            Constraint::Percentage(35),
            Constraint::Percentage(37),
        ],
    )
    .header(
        Row::new(["", "Key", "Value", "Description"])
            .style(muted_style().add_modifier(Modifier::BOLD)),
    )
    .block(block)
    .style(Style::default().fg(TEXT));
    frame.render_widget(t, area);
}
