use std::io::{self, Read};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
};

use crate::project::{HttpMethod, RataProject};

const PANEL: Color = Color::Rgb(24, 27, 34);
const SELECTED_BG: Color = Color::Rgb(55, 60, 75);
const BORDER: Color = Color::Rgb(62, 68, 82);
const TEXT: Color = Color::Rgb(242, 244, 247);
const MUTED: Color = Color::Rgb(152, 162, 179);
const ACCENT: Color = Color::Rgb(255, 138, 95);
const GREEN: Color = Color::Rgb(47, 209, 124);
const YELLOW: Color = Color::Rgb(245, 184, 75);
const RED: Color = Color::Rgb(255, 123, 114);
const BLUE: Color = Color::Rgb(96, 165, 250);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiModel {
    pub theme: Theme,
    pub collections_title: String,
    pub selected_request_url: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestDraft {
    pub method: HttpMethod,
    pub url: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseView {
    pub status: Option<u16>,
    pub body: String,
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseTab {
    Body,
    Headers,
    Cookies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    EditingUrl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveBlock {
    None,
    Collections,
    Request,
    Params,
    Examples,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    None,
    Collections,
    Request,
    Response,
    Examples,
}

#[derive(Debug, Clone)]
pub struct TuiApp {
    pub model: TuiModel,
    pub draft: RequestDraft,
    pub response: ResponseView,
    pub input_mode: InputMode,
    pub active_response_tab: ResponseTab,
    response_tab_origin: (u16, u16),
    pub collections_area: Rect,
    pub request_area: Rect,
    pub params_area: Rect,
    pub examples_area: Rect,
    pub response_area: Rect,
    pub collapsed_tags: std::collections::HashSet<String>,
    pub active_block: ActiveBlock,
    pub selected_operation: Option<(HttpMethod, String)>,
    pub response_scroll: u16,
    pub collections_width: u16,
    pub request_height: u16,
    pub response_height_percent: u16,
    pub examples_width: u16,
    pub drag_target: DragTarget,
}

impl TuiApp {
    pub fn new(project: Option<&RataProject>) -> Self {
        let model = build_model(project);
        let method = project
            .and_then(first_operation)
            .map(|operation| operation.method)
            .unwrap_or(HttpMethod::Get);
        let selected_operation = project
            .and_then(first_operation)
            .map(|op| (op.method, op.path.clone()));

        Self {
            draft: RequestDraft {
                method,
                url: model.selected_request_url.clone(),
                body: String::new(),
            },
            model,
            response: ResponseView {
                status: None,
                body: String::new(),
                headers: Vec::new(),
                cookies: Vec::new(),
                error: None,
            },
            input_mode: InputMode::Normal,
            active_response_tab: ResponseTab::Body,
            response_tab_origin: (0, RESPONSE_TAB_ROW),
            collections_area: Rect::default(),
            request_area: Rect::default(),
            params_area: Rect::default(),
            examples_area: Rect::default(),
            response_area: Rect::default(),
            collapsed_tags: std::collections::HashSet::new(),
            active_block: ActiveBlock::None,
            selected_operation,
            response_scroll: 0,
            collections_width: 34,
            request_height: 6,
            response_height_percent: 66,
            examples_width: 34,
            drag_target: DragTarget::None,
        }
    }

    pub fn scroll_response_up(&mut self, amount: u16) {
        self.response_scroll = self.response_scroll.saturating_sub(amount);
    }

    pub fn scroll_response_down(&mut self, amount: u16) {
        let lines = if let Some(error) = &self.response.error {
            error.lines().count() as u16
        } else {
            match self.active_response_tab {
                ResponseTab::Body => pretty_body(&self.response.body).lines().count() as u16,
                ResponseTab::Headers => self.response.headers.len().max(1) as u16,
                ResponseTab::Cookies => self.response.cookies.len().max(1) as u16,
            }
        };
        let view_height = self.response_area.height.saturating_sub(2);
        let max_scroll = lines.saturating_sub(view_height);
        self.response_scroll = std::cmp::min(self.response_scroll.saturating_add(amount), max_scroll);
    }

    pub fn edit_url(&mut self, url: impl Into<String>) {
        self.draft.url = url.into();
    }

    pub fn send(&mut self) -> anyhow::Result<()> {
        self.response_scroll = 0;
        self.response = ResponseView {
            status: None,
            body: String::new(),
            headers: Vec::new(),
            cookies: Vec::new(),
            error: None,
        };

        match self.send_request() {
            Ok(response) => self.response = response,
            Err(error) => {
                self.response.error = Some(error.to_string());
                return Err(error);
            }
        }

        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent, project: Option<&RataProject>) -> anyhow::Result<AppAction> {
        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => Ok(AppAction::Quit),
                KeyCode::Char('e') | KeyCode::Char('i') => {
                    self.input_mode = InputMode::EditingUrl;
                    Ok(AppAction::Continue)
                }
                KeyCode::Enter | KeyCode::Char('s') => {
                    let _ = self.send();
                    Ok(AppAction::Continue)
                }
                KeyCode::Char('1') => {
                    self.active_response_tab = ResponseTab::Body;
                    self.response_scroll = 0;
                    Ok(AppAction::Continue)
                }
                KeyCode::Char('2') => {
                    self.active_response_tab = ResponseTab::Headers;
                    self.response_scroll = 0;
                    Ok(AppAction::Continue)
                }
                KeyCode::Char('3') => {
                    self.active_response_tab = ResponseTab::Cookies;
                    self.response_scroll = 0;
                    Ok(AppAction::Continue)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.active_block == ActiveBlock::Response {
                        self.scroll_response_up(1);
                    } else if self.active_block == ActiveBlock::Collections {
                        self.select_previous_operation(project);
                    }
                    Ok(AppAction::Continue)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.active_block == ActiveBlock::Response {
                        self.scroll_response_down(1);
                    } else if self.active_block == ActiveBlock::Collections {
                        self.select_next_operation(project);
                    }
                    Ok(AppAction::Continue)
                }
                _ => Ok(AppAction::Continue),
            },
            InputMode::EditingUrl => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    Ok(AppAction::Continue)
                }
                KeyCode::Enter => {
                    self.input_mode = InputMode::Normal;
                    let _ = self.send();
                    Ok(AppAction::Continue)
                }
                KeyCode::Backspace => {
                    self.draft.url.pop();
                    Ok(AppAction::Continue)
                }
                KeyCode::Char(value) => {
                    self.draft.url.push(value);
                    Ok(AppAction::Continue)
                }
                _ => Ok(AppAction::Continue),
            },
        }
    }

    fn select_operation(&mut self, operation: &crate::project::Operation, project: &RataProject) {
        self.selected_operation = Some((operation.method, operation.path.clone()));
        let base = project.server_url().unwrap_or_default().trim_end_matches('/');
        self.draft.method = operation.method;
        self.draft.url = format!("{base}{}", operation.path);
        self.model.examples = project.examples_for(operation).ok().unwrap_or_default().into_iter().map(|e| e.name).collect();
    }

    fn get_visible_operations<'a>(&self, project: &'a RataProject) -> Vec<&'a crate::project::Operation> {
        let mut ops = Vec::new();
        for collection in project.collections() {
            if !self.collapsed_tags.contains(&collection.name) {
                for operation in &collection.operations {
                    ops.push(operation);
                }
            }
        }
        ops
    }

    fn select_next_operation(&mut self, project: Option<&RataProject>) {
        if let Some(project) = project {
            let ops = self.get_visible_operations(project);
            if ops.is_empty() { return; }
            let mut next_op = ops[0];
            if let Some(selected) = &self.selected_operation {
                if let Some(pos) = ops.iter().position(|op| op.method == selected.0 && op.path == selected.1) {
                    if pos + 1 < ops.len() {
                        next_op = ops[pos + 1];
                    } else {
                        next_op = ops[pos]; // stay at last
                    }
                }
            }
            self.select_operation(next_op, project);
        }
    }

    fn select_previous_operation(&mut self, project: Option<&RataProject>) {
        if let Some(project) = project {
            let ops = self.get_visible_operations(project);
            if ops.is_empty() { return; }
            let mut prev_op = ops[0];
            if let Some(selected) = &self.selected_operation {
                if let Some(pos) = ops.iter().position(|op| op.method == selected.0 && op.path == selected.1) {
                    if pos > 0 {
                        prev_op = ops[pos - 1];
                    } else {
                        prev_op = ops[0]; // stay at first
                    }
                }
            }
            self.select_operation(prev_op, project);
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, project: Option<&RataProject>) {
        let contains = |rect: Rect, x, y| {
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
        };

        if matches!(mouse.kind, MouseEventKind::ScrollUp) {
            if contains(self.response_area, mouse.column, mouse.row) {
                self.active_block = ActiveBlock::Response;
                self.scroll_response_up(3);
            }
            return;
        }

        if matches!(mouse.kind, MouseEventKind::ScrollDown) {
            if contains(self.response_area, mouse.column, mouse.row) {
                self.active_block = ActiveBlock::Response;
                self.scroll_response_down(3);
            }
            return;
        }

        match mouse.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                match self.drag_target {
                    DragTarget::Collections => {
                        self.collections_width = mouse.column.max(10).min(120);
                    }
                    DragTarget::Request => {
                        self.request_height = mouse.row.saturating_sub(self.request_area.y).max(3).min(20);
                    }
                    DragTarget::Response => {
                        let main_rest_y = self.params_area.y;
                        let main_rest_h = self.params_area.height.saturating_add(self.response_area.height);
                        if main_rest_h > 0 {
                            let offset = mouse.row.saturating_sub(main_rest_y);
                            let percent = (offset as u32 * 100 / main_rest_h as u32) as u16;
                            self.response_height_percent = 100u16.saturating_sub(percent).max(10).min(90);
                        }
                    }
                    DragTarget::Examples => {
                        self.examples_width = self.examples_area.right().saturating_sub(mouse.column).max(10).min(120);
                    }
                    DragTarget::None => {}
                }
                return;
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag_target = DragTarget::None;
                return;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(tab) = response_tab_at(mouse.column, mouse.row, self.response_tab_origin) {
                    self.active_response_tab = tab;
                    self.response_scroll = 0;
                    self.active_block = ActiveBlock::Response;
                    return;
                }

                if mouse.column == self.collections_area.right().saturating_sub(1) || mouse.column == self.collections_area.right() {
                    self.drag_target = DragTarget::Collections;
                    return;
                }
                if mouse.row == self.params_area.y.saturating_sub(1) || mouse.row == self.params_area.y {
                    self.drag_target = DragTarget::Request;
                    return;
                }
                if mouse.row == self.response_area.y.saturating_sub(1) || mouse.row == self.response_area.y {
                    self.drag_target = DragTarget::Response;
                    return;
                }
                if mouse.column == self.examples_area.x.saturating_sub(1) || mouse.column == self.examples_area.x {
                    self.drag_target = DragTarget::Examples;
                    return;
                }
            }
            _ => return,
        }

        self.active_block = ActiveBlock::None;

        if contains(self.collections_area, mouse.column, mouse.row) {
            self.active_block = ActiveBlock::Collections;
            let list_y = self.collections_area.y + 1;
            if mouse.row >= list_y {
                let clicked_row = (mouse.row - list_y) as usize;
                if let Some(project) = project {
                    let mut current_row = 0;
                    for collection in project.collections() {
                        if current_row == clicked_row {
                            let name = collection.name.clone();
                            if self.collapsed_tags.contains(&name) {
                                self.collapsed_tags.remove(&name);
                            } else {
                                self.collapsed_tags.insert(name);
                            }
                            return;
                        }
                        current_row += 1;
                        if !self.collapsed_tags.contains(&collection.name) {
                            let ops_len = collection.operations.len();
                            if clicked_row >= current_row && clicked_row < current_row + ops_len {
                                let op_idx = clicked_row - current_row;
                                let operation = &collection.operations[op_idx];
                                self.select_operation(operation, project);
                                return;
                            }
                            current_row += ops_len;
                        }
                    }
                }
            }
        } else if contains(self.request_area, mouse.column, mouse.row) {
            self.active_block = ActiveBlock::Request;
        } else if contains(self.params_area, mouse.column, mouse.row) {
            self.active_block = ActiveBlock::Params;
        } else if contains(self.examples_area, mouse.column, mouse.row) {
            self.active_block = ActiveBlock::Examples;
        } else if contains(self.response_area, mouse.column, mouse.row) {
            self.active_block = ActiveBlock::Response;
        }
    }

    fn set_response_tabs_area(&mut self, area: Rect) {
        self.response_tab_origin = (area.x, area.y);
    }

    pub fn response_tabs(&self) -> [&'static str; 3] {
        ["Body", "Headers", "Cookies"]
    }

    pub fn active_response_text(&self) -> Text<'static> {
        if let Some(error) = &self.response.error {
            return Text::raw(error.clone());
        }

        match self.active_response_tab {
            ResponseTab::Body => {
                let body = pretty_body(&self.response.body);
                highlight_json(&body)
            }
            ResponseTab::Headers => {
                Text::raw(format_pairs(&self.response.headers, "No headers"))
            }
            ResponseTab::Cookies => {
                if self.response.cookies.is_empty() {
                    Text::raw("No cookies".to_string())
                } else {
                    Text::raw(self.response.cookies.join("\n"))
                }
            }
        }
    }

    fn send_request(&self) -> anyhow::Result<ResponseView> {
        let client = reqwest::blocking::Client::new();
        let mut response = client
            .request(self.draft.method.reqwest(), &self.draft.url)
            .body(self.draft.body.clone())
            .send()?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    value.to_str().unwrap_or("<binary>").to_string(),
                )
            })
            .collect::<Vec<_>>();
        let cookies = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap_or("<binary>").to_string())
            .collect::<Vec<_>>();
        let mut body = String::new();
        response.read_to_string(&mut body)?;

        Ok(ResponseView {
            status: Some(status),
            body,
            headers,
            cookies,
            error: None,
        })
    }
}

fn response_tab_at(column: u16, row: u16, origin: (u16, u16)) -> Option<ResponseTab> {
    let (origin_column, origin_row) = origin;
    if row != origin_row || column < origin_column {
        return None;
    }

    match column - origin_column {
        BODY_TAB_START..=BODY_TAB_END => Some(ResponseTab::Body),
        HEADERS_TAB_START..=HEADERS_TAB_END => Some(ResponseTab::Headers),
        COOKIES_TAB_START..=COOKIES_TAB_END => Some(ResponseTab::Cookies),
        _ => None,
    }
}

const RESPONSE_TAB_ROW: u16 = 3;
const BODY_TAB_START: u16 = 2;
const BODY_TAB_END: u16 = 5;
const HEADERS_TAB_START: u16 = 9;
const HEADERS_TAB_END: u16 = 15;
const COOKIES_TAB_START: u16 = 19;
const COOKIES_TAB_END: u16 = 25;

fn pretty_body(body: &str) -> String {
    if body.is_empty() {
        return "No response yet.".to_string();
    }

    serde_json::from_str::<serde_json::Value>(body)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| body.to_string())
}

fn format_pairs(pairs: &[(String, String)], empty: &str) -> String {
    if pairs.is_empty() {
        return empty.to_string();
    }

    pairs
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn first_operation(project: &RataProject) -> Option<&crate::project::Operation> {
    project
        .collections()
        .iter()
        .flat_map(|collection| &collection.operations)
        .next()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
}

pub fn build_model(project: Option<&RataProject>) -> TuiModel {
    let Some(project) = project else {
        return TuiModel {
            theme: Theme::Dark,
            collections_title: "Collections".to_string(),
            selected_request_url: String::new(),
            examples: Vec::new(),
        };
    };

    let operations = project
        .collections()
        .iter()
        .flat_map(|collection| &collection.operations)
        .collect::<Vec<_>>();
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
        selected_request_url,
        examples,
    }
}

pub fn run(project: Option<&RataProject>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new(project);
    let result = run_loop(&mut terminal, project, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    project: Option<&RataProject>,
    app: &mut TuiApp,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(app.collections_width), Constraint::Min(0)])
                .split(area);
            app.collections_area = body[0];
            let main = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(app.request_height),
                    Constraint::Min(0),
                ])
                .split(body[1]);
            let main_rest = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(100_u16.saturating_sub(app.response_height_percent)),
                    Constraint::Percentage(app.response_height_percent),
                ])
                .split(main[1]);
            let request_body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(app.examples_width)])
                .split(main_rest[0]);

            app.request_area = main[0];
            app.params_area = request_body[0];
            app.examples_area = request_body[1];
            app.response_area = main_rest[1];

            frame.render_widget(collections(project, app), body[0]);
            frame.render_widget(request_line(app), main[0]);
            frame.render_widget(params_table(app), request_body[0]);
            frame.render_widget(examples(project, app), request_body[1]);
            render_response(frame, app, main_rest[1]);
        })?;

        if event::poll(std::time::Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if app.handle_key(key, project)? == AppAction::Quit => return Ok(()),
                Event::Mouse(mouse) => app.handle_mouse(mouse, project),
                _ => {}
            }
        }
    }
}



fn collections(project: Option<&RataProject>, app: &TuiApp) -> List<'static> {
    let mut items = Vec::new();
    let collapsed = &app.collapsed_tags;
    let border_style = if app.active_block == ActiveBlock::Collections {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    if let Some(project) = project {
        for collection in project.collections() {
            let is_collapsed = collapsed.contains(&collection.name);
            let icon = if is_collapsed { "▸ " } else { "▾ " };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(icon, muted_style()),
                Span::styled(
                    format!("{}/", collection.name),
                    accent_style().add_modifier(Modifier::BOLD),
                ),
            ])));
            if !is_collapsed {
                for operation in &collection.operations {
                    let is_selected = app.selected_operation.as_ref() == Some(&(operation.method, operation.path.clone()));
                    let mut item = ListItem::new(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("{:<5}", operation.method.label()),
                            method_style(operation.method),
                        ),
                        Span::styled(operation.summary.clone(), Style::default().fg(TEXT)),
                    ]));
                    if is_selected {
                        item = item.style(Style::default().bg(SELECTED_BG));
                    }
                    items.push(item);
                }
            }
        }
    } else {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "No .rata/openapi.yaml found",
            muted_style(),
        )])));
    }

    List::new(items)
        .block(
            Block::default()
                .title(" Collections ")
                .borders(Borders::ALL)
                .style(Style::default().bg(PANEL).fg(TEXT))
                .border_style(border_style)
        )
        .style(Style::default().fg(TEXT))
        .highlight_style(Style::default().bg(SELECTED_BG))
}



fn request_line(app: &TuiApp) -> Paragraph<'static> {
    let url = if app.draft.url.is_empty() {
        "No request selected".to_string()
    } else {
        app.draft.url.clone()
    };
    let mode_hint = match app.input_mode {
        InputMode::Normal => "e edit URL · Enter/s send · q quit",
        InputMode::EditingUrl => {
            "typing edits URL · Backspace delete · Enter send · Esc cancel edit"
        }
    };

    let border_style = if app.active_block == ActiveBlock::Request {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    Paragraph::new(vec![
        Line::from(Span::styled(
            app.draft.method.label(),
            method_style(app.draft.method),
        )),
        Line::from(Span::styled(url, Style::default().fg(TEXT))),
        Line::from(Span::styled(mode_hint, muted_style())),
    ])
    .block(
        Block::default()
            .title(" Request ")
            .borders(Borders::ALL)
            .style(Style::default().bg(PANEL).fg(TEXT))
            .border_style(border_style)
    )
}

fn params_table(app: &TuiApp) -> Table<'static> {
    let border_style = if app.active_block == ActiveBlock::Params {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

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
            .style(muted_style().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .title(" Params ")
            .borders(Borders::ALL)
            .style(Style::default().bg(PANEL).fg(TEXT))
            .border_style(border_style)
    )
    .style(Style::default().fg(TEXT))
}

fn examples(project: Option<&RataProject>, app: &TuiApp) -> List<'static> {
    let border_style = if app.active_block == ActiveBlock::Examples {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    let model = build_model(project);
    let items = if model.examples.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No examples",
            muted_style(),
        )))]
    } else {
        model
            .examples
            .into_iter()
            .map(|example| {
                ListItem::new(Line::from(vec![
                    Span::styled("• ", accent_style()),
                    Span::styled(example, Style::default().fg(TEXT)),
                ]))
            })
            .collect()
    };

    List::new(items)
        .block(
            Block::default()
                .title(" Examples ")
                .borders(Borders::ALL)
                .style(Style::default().bg(PANEL).fg(TEXT))
                .border_style(border_style)
        )
        .style(Style::default().fg(TEXT))
}

fn render_response(frame: &mut ratatui::Frame<'_>, app: &mut TuiApp, area: Rect) {
    let block = response_block(app);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    app.set_response_tabs_area(Rect { x: area.x, y: area.y, width: area.width, height: 1 });
    
    let text = app.active_response_text();
    let lines = text.lines.len();
    let view_height = inner.height as usize;

    frame.render_widget(response_body(app, text), inner);

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
            area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

fn response_tabs_title(app: &TuiApp) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(Span::raw(" "));
    let tabs = app.response_tabs();
    let selected = match app.active_response_tab {
        ResponseTab::Body => 0,
        ResponseTab::Headers => 1,
        ResponseTab::Cookies => 2,
    };
    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", muted_style()));
        }
        if i == selected {
            spans.push(Span::styled(*tab, accent_style().add_modifier(Modifier::BOLD)));
        } else {
            spans.push(Span::styled(*tab, muted_style()));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans).left_aligned()
}

fn response_status_title(app: &TuiApp) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled("Response", Style::default().fg(TEXT)),
        Span::styled(response_status_label(app), muted_style()),
        Span::raw(" "),
    ]).right_aligned()
}

fn response_body(app: &TuiApp, text: Text<'static>) -> Paragraph<'static> {
    Paragraph::new(text)
        .style(Style::default().bg(PANEL).fg(TEXT))
        .scroll((app.response_scroll, 0))
}

fn response_block(app: &TuiApp) -> Block<'static> {
    let border_style = if app.active_block == ActiveBlock::Response {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    Block::default()
        .style(Style::default().bg(PANEL).fg(TEXT))
        .border_style(border_style)
        .title_top(response_tabs_title(app))
        .title_top(response_status_title(app))
        .borders(Borders::ALL)
}

fn response_status_label(app: &TuiApp) -> String {
    app.response
        .status
        .map(|status| format!(" · HTTP {status}"))
        .unwrap_or_default()
}

fn muted_style() -> Style {
    Style::default().fg(MUTED)
}

fn accent_style() -> Style {
    Style::default().fg(ACCENT)
}

fn method_style(method: HttpMethod) -> Style {
    let color = match method {
        HttpMethod::Get => GREEN,
        HttpMethod::Post => YELLOW,
        HttpMethod::Put | HttpMethod::Patch => BLUE,
        HttpMethod::Delete => RED,
    };

    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn highlight_json(json: &str) -> Text<'static> {
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
                        if lc == ' ' { continue; }
                        if lc == ':' { is_key_local = true; }
                        break;
                    }
                    if is_key_local {
                        spans.push(Span::styled(current_span.clone(), Style::default().fg(Color::LightBlue)));
                    } else {
                        spans.push(Span::styled(current_span.clone(), Style::default().fg(Color::Green)));
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
                spans.push(Span::styled(current_span, Style::default().fg(Color::Green)));
            } else {
                spans.extend(highlight_non_string(&current_span));
            }
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

fn highlight_non_string(text: &str) -> Vec<Span<'static>> {
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

fn highlight_value(text: &str) -> Span<'static> {
    match text {
        "true" | "false" => Span::styled(text.to_string(), Style::default().fg(Color::Yellow)),
        "null" => Span::styled(text.to_string(), Style::default().fg(Color::DarkGray)),
        _ if text.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+') => {
            Span::styled(text.to_string(), Style::default().fg(Color::Magenta))
        }
        _ => Span::raw(text.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Removed tests for non-existent functions response_tabs_widget and response_tabs_area

    #[test]
    fn http_methods_have_distinct_styles() {
        assert_eq!(
            method_style(HttpMethod::Get).fg,
            Some(Color::Rgb(47, 209, 124))
        );
        assert_eq!(
            method_style(HttpMethod::Post).fg,
            Some(Color::Rgb(245, 184, 75))
        );
        assert_eq!(
            method_style(HttpMethod::Delete).fg,
            Some(Color::Rgb(255, 123, 114))
        );
    }
}
