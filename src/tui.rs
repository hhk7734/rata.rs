use std::io;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Tabs},
};

use crate::project::RataProject;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiModel {
    pub theme: Theme,
    pub collections_title: String,
    pub request_tabs: Vec<String>,
    pub selected_request_url: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
}

pub fn build_model(project: Option<&RataProject>) -> TuiModel {
    let Some(project) = project else {
        return TuiModel {
            theme: Theme::Dark,
            collections_title: "Collections".to_string(),
            request_tabs: Vec::new(),
            selected_request_url: String::new(),
            examples: Vec::new(),
        };
    };

    let operations = project
        .collections()
        .iter()
        .flat_map(|collection| &collection.operations)
        .collect::<Vec<_>>();
    let request_tabs = operations
        .iter()
        .map(|operation| format!("{} {}", operation.method.label(), operation.summary))
        .collect();
    let selected = operations.first().copied();
    let selected_request_url = selected
        .map(|operation| {
            let base = project
                .server_url()
                .unwrap_or_default()
                .trim_end_matches('/');
            format!("{base}{}", operation.path)
        })
        .unwrap_or_default();
    let examples = selected
        .and_then(|operation| project.examples_for(operation).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|example| example.name)
        .collect();

    TuiModel {
        theme: Theme::Dark,
        collections_title: "Collections".to_string(),
        request_tabs,
        selected_request_url,
        examples,
    }
}

pub fn run(project: Option<&RataProject>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, project);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    project: Option<&RataProject>,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(area);
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(34), Constraint::Min(0)])
                .split(vertical[1]);
            let main = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(6),
                    Constraint::Min(8),
                    Constraint::Length(10),
                ])
                .split(body[1]);
            let request_body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(34)])
                .split(main[2]);

            frame.render_widget(top_bar(), vertical[0]);
            frame.render_widget(collections(project), body[0]);
            frame.render_widget(request_tabs(project), main[0]);
            frame.render_widget(request_line(project), main[1]);
            frame.render_widget(params_table(), request_body[0]);
            frame.render_widget(examples(project), request_body[1]);
            frame.render_widget(response_panel(), main[3]);
        })?;

        if event::poll(std::time::Duration::from_millis(250))?
            && matches!(
                event::read()?,
                Event::Key(key) if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
            )
        {
            return Ok(());
        }
    }
}

fn top_bar() -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled(
            " rata ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(255, 108, 55)),
        ),
        Span::raw("  Example API  "),
        Span::styled(
            "Search operations, examples, paths",
            Style::default().fg(Color::Rgb(152, 162, 179)),
        ),
    ]))
    .block(dark_block())
}

fn collections(project: Option<&RataProject>) -> List<'static> {
    let mut items = Vec::new();
    if let Some(project) = project {
        for collection in project.collections() {
            items.push(
                ListItem::new(format!("v {}/", collection.name)).style(
                    Style::default()
                        .fg(Color::Rgb(242, 244, 247))
                        .add_modifier(Modifier::BOLD),
                ),
            );
            for operation in &collection.operations {
                items.push(ListItem::new(format!(
                    "  {:<5} {}",
                    operation.method.label(),
                    operation.summary
                )));
            }
        }
    } else {
        items.push(ListItem::new("No .rata/openapi.yaml found"));
    }

    List::new(items)
        .block(dark_block().title("Collections").borders(Borders::ALL))
        .style(Style::default().fg(Color::Rgb(242, 244, 247)))
        .highlight_style(Style::default().bg(Color::Rgb(51, 36, 29)))
}

fn request_tabs(project: Option<&RataProject>) -> Tabs<'static> {
    let model = build_model(project);
    let titles = if model.request_tabs.is_empty() {
        vec![Line::from("Request")]
    } else {
        model.request_tabs.into_iter().map(Line::from).collect()
    };

    Tabs::new(titles)
        .select(0)
        .block(dark_block())
        .style(Style::default().fg(Color::Rgb(152, 162, 179)))
        .highlight_style(
            Style::default()
                .fg(Color::Rgb(255, 138, 95))
                .add_modifier(Modifier::BOLD),
        )
}

fn request_line(project: Option<&RataProject>) -> Paragraph<'static> {
    let model = build_model(project);
    let url = if model.selected_request_url.is_empty() {
        "No request selected".to_string()
    } else {
        model.selected_request_url
    };

    Paragraph::new(vec![
        Line::from(Span::styled(
            "GET",
            Style::default()
                .fg(Color::Rgb(47, 209, 124))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(url),
        Line::from(Span::styled(
            "Send with CLI mode: rata <url>",
            Style::default().fg(Color::Rgb(152, 162, 179)),
        )),
    ])
    .block(dark_block().title("Request").borders(Borders::ALL))
}

fn params_table() -> Table<'static> {
    Table::new(
        [
            Row::new(["id", "{id}", "Path", "OpenAPI path parameter"]),
            Row::new([
                "accept",
                "application/json",
                "Header",
                "Default response format",
            ]),
        ],
        [
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Percentage(18),
            Constraint::Percentage(37),
        ],
    )
    .header(
        Row::new(["Key", "Value", "Source", "Description"])
            .style(Style::default().fg(Color::Rgb(152, 162, 179))),
    )
    .block(dark_block().title("Params").borders(Borders::ALL))
    .style(Style::default().fg(Color::Rgb(242, 244, 247)))
}

fn examples(project: Option<&RataProject>) -> List<'static> {
    let model = build_model(project);
    let items = if model.examples.is_empty() {
        vec![ListItem::new("No examples")]
    } else {
        model.examples.into_iter().map(ListItem::new).collect()
    };

    List::new(items)
        .block(dark_block().title("Examples").borders(Borders::ALL))
        .style(Style::default().fg(Color::Rgb(242, 244, 247)))
}

fn response_panel() -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(Span::styled(
            "Response",
            Style::default()
                .fg(Color::Rgb(242, 244, 247))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("Run a request from CLI mode to fetch live response output."),
        Line::from("Examples from .rata/<path>/<method>/*.yaml appear in the side panel."),
    ])
    .block(dark_block().title("Response").borders(Borders::ALL))
    .style(Style::default().fg(Color::Rgb(242, 244, 247)))
}

fn dark_block() -> Block<'static> {
    Block::default()
        .style(
            Style::default()
                .bg(Color::Rgb(24, 27, 34))
                .fg(Color::Rgb(242, 244, 247)),
        )
        .border_style(Style::default().fg(Color::Rgb(43, 48, 59)))
}
