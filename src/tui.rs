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

pub const PANEL: Color = Color::Rgb(30, 30, 35);
const SELECTED_BG: Color = Color::Rgb(55, 60, 75);
const BORDER: Color = Color::Rgb(62, 68, 82);
pub const TEXT: Color = Color::Rgb(242, 244, 247);
const MUTED: Color = Color::Rgb(152, 162, 179);
pub const ACCENT: Color = Color::Rgb(255, 138, 95);
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
pub struct ParamState {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamEditMode {
    None,
    Key,
    Value,
}

#[derive(Debug, Clone)]
pub struct RequestDraft {
    pub method: HttpMethod,
    pub url: String,
    pub body: String,
    pub params: Vec<ParamState>,
    pub headers: Vec<ParamState>,
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
pub enum RequestTab {
    Body,
    Query,
    Headers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseTab {
    Body,
    Headers,
    Cookies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveBlock {
    None,
    Collections,
    Request,
    Params,
    Examples,
    Response,
    MethodDropdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    None,
    Collections,
    Request,
    Response,
    ScrollRequest,
    ScrollResponse,
    ResponseSelection,
    RequestSelection,
    ErrorPopupSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

pub struct TuiApp {
    pub model: TuiModel,
    pub draft: RequestDraft,
    pub response: ResponseView,
    pub active_request_tab: RequestTab,
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
    pub examples_dropdown_open: bool,
    pub wrap_body: bool,
    pub drag_target: DragTarget,
    pub selected_request_row: usize,
    pub editing_param_key: Option<String>,
    pub param_edit_mode: ParamEditMode,
    pub text_cursor: usize,
    pub selected_example_row: usize,
    pub request_scroll: u16,
    pub drag_last_row: Option<u16>,
    pub text_selection: Option<Selection>,
    pub request_selection: Option<Selection>,
    pub clipboard: Option<arboard::Clipboard>,
    pub method_dropdown_open: bool,
    pub selected_method_row: usize,
    pub method_dropdown_area: Rect,
    pub is_sending: bool,
    pub send_button_area: Rect,
    pub send_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<ResponseView>>>,
    pub sending_frame: usize,
    pub error_popup: Option<String>,
    pub error_popup_area: Rect,
    pub last_cursor_activity: std::time::Instant,
}

const METHODS: [HttpMethod; 5] = [
    HttpMethod::Get,
    HttpMethod::Post,
    HttpMethod::Put,
    HttpMethod::Patch,
    HttpMethod::Delete,
];

fn count_display_lines(text: &str, wrap: bool, width: usize) -> u16 {
    count_visual_lines(text, width, wrap) as u16
}

fn visual_to_char_index(
    text: &str,
    target_v_line: usize,
    target_v_col: usize,
    width: usize,
    wrap: bool,
) -> usize {
    if !wrap || width == 0 {
        let mut index = 0;
        let mut current_v_line = 0;
        for line in text.lines() {
            let line_len = line.chars().count();
            if current_v_line == target_v_line {
                return index + target_v_col.min(line_len);
            }
            current_v_line += 1;
            index += line_len + 1;
        }
        return text.chars().count();
    }

    let mut current_v_line = 0;
    let mut index = 0;

    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            if current_v_line == target_v_line {
                return index;
            }
            current_v_line += 1;
            index += 1;
            continue;
        }

        let mut line_width = 0;
        let mut char_iter_idx = 0;
        let mut visual_line_start_idx = index;

        while char_iter_idx < chars.len() {
            let mut word_end = char_iter_idx;
            while word_end < chars.len() && !chars[word_end].is_whitespace() {
                word_end += 1;
            }
            let mut ws_end = word_end;
            while ws_end < chars.len() && chars[ws_end].is_whitespace() {
                ws_end += 1;
            }

            let word_len = word_end - char_iter_idx;
            let ws_len = ws_end - word_end;

            if line_width + word_len > width {
                if line_width > 0 {
                    current_v_line += 1;
                    if current_v_line > target_v_line {
                        return visual_line_start_idx + target_v_col.min(line_width);
                    }
                    line_width = 0;
                    visual_line_start_idx = index + char_iter_idx;
                }
            }

            let mut remaining_word = word_len;
            let mut remaining_ws = ws_len;

            while line_width + remaining_word > width {
                let fit = width - line_width;
                remaining_word -= fit;
                char_iter_idx += fit;

                current_v_line += 1;
                if current_v_line > target_v_line {
                    return visual_line_start_idx + target_v_col.min(width);
                }
                line_width = 0;
                visual_line_start_idx = index + char_iter_idx;
            }

            line_width += remaining_word;
            char_iter_idx += remaining_word;

            while line_width + remaining_ws > width {
                let fit = width - line_width;
                remaining_ws -= fit;
                char_iter_idx += fit;

                current_v_line += 1;
                if current_v_line > target_v_line {
                    return visual_line_start_idx + target_v_col.min(width);
                }
                line_width = 0;
                visual_line_start_idx = index + char_iter_idx;
            }

            line_width += remaining_ws;
            char_iter_idx += remaining_ws;
        }

        if current_v_line == target_v_line {
            return visual_line_start_idx + target_v_col.min(line_width);
        }
        current_v_line += 1;
        index += chars.len() + 1;
    }

    text.chars().count()
}

pub fn count_visual_lines(text: &str, width: usize, wrap: bool) -> usize {
    if !wrap || width == 0 {
        return text.lines().count();
    }
    let mut current_v_line = 0;
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            current_v_line += 1;
            continue;
        }

        let mut line_width = 0;
        let mut char_iter_idx = 0;

        while char_iter_idx < chars.len() {
            let mut word_end = char_iter_idx;
            while word_end < chars.len() && !chars[word_end].is_whitespace() {
                word_end += 1;
            }
            let mut ws_end = word_end;
            while ws_end < chars.len() && chars[ws_end].is_whitespace() {
                ws_end += 1;
            }

            let word_len = word_end - char_iter_idx;
            let ws_len = ws_end - word_end;

            if line_width + word_len > width && line_width > 0 {
                current_v_line += 1;
                line_width = 0;
            }

            let mut remaining_word = word_len;
            let mut remaining_ws = ws_len;

            while line_width + remaining_word > width {
                let fit = width - line_width;
                remaining_word -= fit;
                char_iter_idx += fit;

                current_v_line += 1;
                line_width = 0;
            }

            line_width += remaining_word;
            char_iter_idx += remaining_word;

            while line_width + remaining_ws > width {
                let fit = width - line_width;
                remaining_ws -= fit;
                char_iter_idx += fit;

                current_v_line += 1;
                line_width = 0;
            }

            line_width += remaining_ws;
            char_iter_idx += remaining_ws;
        }

        current_v_line += 1;
    }
    current_v_line
}

fn char_index_to_logical(text: &str, target_index: usize) -> (usize, usize) {
    let mut index = 0;
    for (l_idx, line) in text.lines().enumerate() {
        let chars_count = line.chars().count();
        if index + chars_count >= target_index {
            return (l_idx, target_index - index);
        }
        index += chars_count + 1;
    }
    (text.lines().count().saturating_sub(1), usize::MAX)
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
                params: Vec::new(),
                headers: {
                    let mut h = vec![ParamState {
                        key: "user-agent".to_string(),
                        value: format!("rata/{}", env!("CARGO_PKG_VERSION")),
                        enabled: true,
                        required: false,
                    }];
                    if let Some(p) = project {
                        for (k, v) in p.global_headers() {
                            h.push(ParamState {
                                key: k,
                                value: v,
                                enabled: true,
                                required: false,
                            });
                        }
                    }
                    h
                },
            },
            model,
            response: ResponseView {
                status: None,
                body: String::new(),
                headers: Vec::new(),
                cookies: Vec::new(),
                error: None,
            },
            active_request_tab: RequestTab::Query,
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
            request_height: 3,
            response_height_percent: 66,
            examples_dropdown_open: false,
            wrap_body: false,
            drag_target: DragTarget::None,
            selected_request_row: 0,
            editing_param_key: None,
            param_edit_mode: ParamEditMode::None,
            text_cursor: 0,
            selected_example_row: 0,
            request_scroll: 0,
            drag_last_row: None,
            text_selection: None,
            request_selection: None,
            clipboard: arboard::Clipboard::new().ok(),
            method_dropdown_open: false,
            selected_method_row: 0,
            method_dropdown_area: Rect::default(),
            is_sending: false,
            send_button_area: Rect::default(),
            send_rx: None,
            sending_frame: 0,
            error_popup: None,
            error_popup_area: Rect::default(),
            last_cursor_activity: std::time::Instant::now(),
        }
    }

    pub fn cursor_visible(&self) -> bool {
        self.last_cursor_activity.elapsed().as_millis() % 1000 < 500
    }

    pub fn scroll_response_up(&mut self, amount: u16) {
        self.response_scroll = self.response_scroll.saturating_sub(amount);
    }

    pub fn scroll_response_down(&mut self, amount: u16) {
        let lines = if let Some(error) = &self.response.error {
            error.lines().count() as u16
        } else {
            match self.active_response_tab {
                ResponseTab::Body => {
                    let width = self.response_area.width.saturating_sub(2) as usize;
                    count_display_lines(
                        &crate::components::body::pretty_body(&self.response.body),
                        self.wrap_body,
                        width,
                    )
                }
                ResponseTab::Headers => self.response.headers.len().max(1) as u16,
                ResponseTab::Cookies => self.response.cookies.len().max(1) as u16,
            }
        };
        let view_height = self.response_area.height.saturating_sub(2);
        let max_scroll = lines.saturating_sub(view_height);
        self.response_scroll =
            std::cmp::min(self.response_scroll.saturating_add(amount), max_scroll);
    }

    pub fn scroll_request_up(&mut self, amount: u16) {
        self.request_scroll = self.request_scroll.saturating_sub(amount);
    }

    pub fn scroll_request_down(&mut self, amount: u16) {
        if self.active_request_tab == RequestTab::Body {
            let width = self.params_area.width.saturating_sub(2) as usize;
            let lines = count_display_lines(&self.draft.body, self.wrap_body, width);
            let view_height = self.params_area.height.saturating_sub(2);
            let max_scroll = lines.saturating_sub(view_height);
            self.request_scroll =
                std::cmp::min(self.request_scroll.saturating_add(amount), max_scroll);
        }
    }

    pub fn ensure_cursor_visible(&mut self) {
        if self.active_request_tab != RequestTab::Body {
            return;
        }

        let width = self.params_area.width.saturating_sub(2) as usize;
        let text_before_cursor = self
            .draft
            .body
            .chars()
            .take(self.text_cursor)
            .collect::<String>();
        let cursor_line =
            count_display_lines(&text_before_cursor, self.wrap_body, width).saturating_sub(1);
        let view_height = self.params_area.height.saturating_sub(2);
        if view_height == 0 {
            return;
        }

        if cursor_line < self.request_scroll {
            self.request_scroll = cursor_line;
        } else if cursor_line >= self.request_scroll + view_height {
            self.request_scroll = cursor_line.saturating_sub(view_height).saturating_add(1);
        }
    }

    pub fn edit_url(&mut self, url: impl Into<String>) {
        self.draft.url = url.into();
    }

    pub fn send(&mut self, project: Option<&RataProject>) {
        if self.is_sending {
            return;
        }

        if let Some(proj) = project {
            if let Some((method, path)) = &self.selected_operation {
                match proj.validate_request_body(*method, path, &self.draft.body) {
                    Ok(errors) => {
                        if !errors.is_empty() {
                            self.error_popup = Some(format!("Request Validation Failed:\n{}", errors.join("\n")));
                            return;
                        }
                    }
                    Err(e) => {
                        self.error_popup = Some(format!("Schema Error:\n{}", e));
                        return;
                    }
                }
            }
        }

        self.response_scroll = 0;
        self.response = ResponseView {
            status: None,
            body: String::new(),
            headers: Vec::new(),
            cookies: Vec::new(),
            error: None,
        };

        let (tx, rx) = std::sync::mpsc::channel();
        let draft = self.draft.clone();
        let project = project.cloned();

        std::thread::spawn(move || {
            let res = execute_request(&draft, project.as_ref());
            let _ = tx.send(res);
        });

        self.send_rx = Some(rx);
        self.sending_frame = 0;
        self.is_sending = true;
    }

    fn get_current_text_len(&self, _project: Option<&RataProject>) -> usize {
        if self.active_block == ActiveBlock::Request {
            return self.draft.url.chars().count();
        } else if self.active_block == ActiveBlock::Params {
            if self.active_request_tab == RequestTab::Body {
                return self.draft.body.chars().count();
            } else if self.param_edit_mode != ParamEditMode::None {
                let map = if self.active_request_tab == RequestTab::Query {
                    &self.draft.params
                } else {
                    &self.draft.headers
                };
                if let Some(param) = map.get(self.selected_request_row) {
                    if self.param_edit_mode == ParamEditMode::Key {
                        return param.key.chars().count();
                    } else if self.param_edit_mode == ParamEditMode::Value {
                        return param.value.chars().count();
                    }
                }
            }
        }
        0
    }
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        project: Option<&RataProject>,
    ) -> anyhow::Result<AppAction> {
        if self.error_popup.is_some() {
            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                self.error_popup = None;
            }
            return Ok(AppAction::Continue);
        }

        if key.code == KeyCode::Char('q')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            return Ok(AppAction::Quit);
        }
        if key.code == KeyCode::Char('e')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            if self.active_block == ActiveBlock::Params
                && self.active_request_tab != RequestTab::Body
            {
                if self.param_edit_mode == ParamEditMode::None {
                    self.param_edit_mode = ParamEditMode::Value;
                    self.text_cursor = usize::MAX;
                } else {
                    self.param_edit_mode = ParamEditMode::None;
                }
            }
            return Ok(AppAction::Continue);
        }
        if key.code == KeyCode::Char('s')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            self.send(project);
            return Ok(AppAction::Continue);
        }
        if key.code == KeyCode::Char('w')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            self.wrap_body = !self.wrap_body;
            return Ok(AppAction::Continue);
        }

        if matches!(
            key.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
        ) {
            self.text_selection = None;
            self.request_selection = None;
        }

        match key.code {
            KeyCode::Esc => {
                if self.param_edit_mode != ParamEditMode::None {
                    self.param_edit_mode = ParamEditMode::None;
                }
                if self.examples_dropdown_open {
                    self.examples_dropdown_open = false;
                    self.active_block = ActiveBlock::Request;
                }
                if self.method_dropdown_open {
                    self.method_dropdown_open = false;
                    self.active_block = ActiveBlock::Request;
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Tab => {
                if self.active_block == ActiveBlock::Params
                    && self.active_request_tab != RequestTab::Body
                    && self.param_edit_mode != ParamEditMode::None
                {
                    let map = if self.active_request_tab == RequestTab::Query {
                        &self.draft.params
                    } else {
                        &self.draft.headers
                    };
                    if let Some(param) = map.get(self.selected_request_row) {
                        if self.param_edit_mode == ParamEditMode::Key {
                            self.param_edit_mode = ParamEditMode::Value;
                        } else if !param.required {
                            self.param_edit_mode = ParamEditMode::Key;
                        }
                        self.text_cursor = usize::MAX;
                    }
                    return Ok(AppAction::Continue);
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Left => {
                let len = self.get_current_text_len(project);
                self.text_cursor = self.text_cursor.min(len).saturating_sub(1);
                self.ensure_cursor_visible();
                Ok(AppAction::Continue)
            }
            KeyCode::Right => {
                let len = self.get_current_text_len(project);
                self.text_cursor = self.text_cursor.min(len).saturating_add(1);
                self.ensure_cursor_visible();
                Ok(AppAction::Continue)
            }
            KeyCode::End => {
                self.text_cursor = if self.active_block == ActiveBlock::Params
                    && self.active_request_tab == RequestTab::Body
                {
                    move_cursor_to_line_end(&self.draft.body, self.text_cursor)
                } else {
                    self.get_current_text_len(project)
                };
                self.ensure_cursor_visible();
                Ok(AppAction::Continue)
            }
            KeyCode::Home => {
                self.text_cursor = if self.active_block == ActiveBlock::Params
                    && self.active_request_tab == RequestTab::Body
                {
                    move_cursor_to_line_start(&self.draft.body, self.text_cursor)
                } else {
                    0
                };
                self.ensure_cursor_visible();
                Ok(AppAction::Continue)
            }
            KeyCode::Up => {
                if self.active_block == ActiveBlock::Response {
                    self.scroll_response_up(1);
                } else if self.active_block == ActiveBlock::Collections {
                    self.select_previous_operation(project);
                } else if self.active_block == ActiveBlock::Params {
                    if self.active_request_tab == RequestTab::Body {
                        self.text_cursor = move_cursor_up(&self.draft.body, self.text_cursor);
                        self.ensure_cursor_visible();
                    } else if self.param_edit_mode == ParamEditMode::None {
                        self.selected_request_row = self.selected_request_row.saturating_sub(1);
                        self.text_cursor = usize::MAX;
                    } else {
                        self.text_cursor = 0;
                    }
                } else if self.active_block == ActiveBlock::Examples {
                    self.selected_example_row = self.selected_example_row.saturating_sub(1);
                } else if self.active_block == ActiveBlock::MethodDropdown {
                    self.selected_method_row = self.selected_method_row.saturating_sub(1);
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Down => {
                if self.active_block == ActiveBlock::Response {
                    self.scroll_response_down(1);
                } else if self.active_block == ActiveBlock::Collections {
                    self.select_next_operation(project);
                } else if self.active_block == ActiveBlock::Params {
                    if self.active_request_tab == RequestTab::Body {
                        self.text_cursor = move_cursor_down(&self.draft.body, self.text_cursor);
                        self.ensure_cursor_visible();
                    } else if self.param_edit_mode == ParamEditMode::None {
                        let max = if self.active_request_tab == RequestTab::Query {
                            self.draft.params.len()
                        } else {
                            self.draft.headers.len()
                        };
                        self.selected_request_row =
                            self.selected_request_row.saturating_add(1).min(max);
                        self.text_cursor = usize::MAX;
                    } else {
                        self.text_cursor = usize::MAX;
                    }
                } else if self.active_block == ActiveBlock::Examples {
                    let max = self.model.examples.len().saturating_sub(1);
                    self.selected_example_row =
                        self.selected_example_row.saturating_add(1).min(max);
                } else if self.active_block == ActiveBlock::MethodDropdown {
                    let max = METHODS.len().saturating_sub(1);
                    self.selected_method_row = self.selected_method_row.saturating_add(1).min(max);
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Enter => {
                if self.active_block == ActiveBlock::Request {
                    self.send(project);
                } else if self.active_block == ActiveBlock::Params
                    && self.active_request_tab == RequestTab::Body
                {
                    insert_char_at(&mut self.draft.body, self.text_cursor, '\n');
                    self.text_cursor += 1;
                    self.ensure_cursor_visible();
                } else if self.active_block == ActiveBlock::Params
                    && self.active_request_tab != RequestTab::Body
                    && self.param_edit_mode == ParamEditMode::None
                {
                    let map = if self.active_request_tab == RequestTab::Query {
                        &mut self.draft.params
                    } else {
                        &mut self.draft.headers
                    };
                    if self.selected_request_row == map.len() {
                        map.push(ParamState {
                            key: String::new(),
                            value: String::new(),
                            enabled: true,
                            required: false,
                        });
                        self.param_edit_mode = ParamEditMode::Key;
                        self.text_cursor = usize::MAX;
                    } else if let Some(param) = map.get_mut(self.selected_request_row) {
                        param.enabled = !param.enabled;
                    }
                } else if self.active_block == ActiveBlock::Examples {
                    if let Some(example_name) = self.model.examples.get(self.selected_example_row) {
                        let name_clone = example_name.clone();
                        if let Some(project) = project {
                            if let Some(selected) = &self.selected_operation {
                                if let Some(op) = project
                                    .collections()
                                    .iter()
                                    .flat_map(|c| &c.operations)
                                    .find(|o| o.method == selected.0 && o.path == selected.1)
                                {
                                    if let Some(example_file) = project
                                        .examples_for(op)
                                        .ok()
                                        .unwrap_or_default()
                                        .iter()
                                        .find(|e| e.name == name_clone)
                                    {
                                        self.load_example(example_file);
                                    }
                                }
                            }
                        }
                    }
                    self.examples_dropdown_open = false;
                    self.active_block = ActiveBlock::Request;
                } else if self.active_block == ActiveBlock::MethodDropdown {
                    if let Some(m) = METHODS.get(self.selected_method_row) {
                        self.draft.method = *m;
                        self.handle_url_edited(project);
                    }
                    self.method_dropdown_open = false;
                    self.active_block = ActiveBlock::Request;
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Backspace => {
                if self.active_block == ActiveBlock::Request {
                    remove_char_at(&mut self.draft.url, self.text_cursor);
                    self.text_cursor = self.text_cursor.saturating_sub(1);
                    self.handle_url_edited(project);
                } else if self.active_block == ActiveBlock::Params {
                    if self.active_request_tab == RequestTab::Body {
                        remove_char_at(&mut self.draft.body, self.text_cursor);
                        self.text_cursor = self.text_cursor.saturating_sub(1);
                    } else if self.param_edit_mode != ParamEditMode::None {
                        let map = if self.active_request_tab == RequestTab::Query {
                            &mut self.draft.params
                        } else {
                            &mut self.draft.headers
                        };
                        if let Some(param) = map.get_mut(self.selected_request_row) {
                            if self.param_edit_mode == ParamEditMode::Key && !param.required {
                                remove_char_at(&mut param.key, self.text_cursor);
                                self.text_cursor = self.text_cursor.saturating_sub(1);
                            } else if self.param_edit_mode == ParamEditMode::Value {
                                remove_char_at(&mut param.value, self.text_cursor);
                                self.text_cursor = self.text_cursor.saturating_sub(1);
                            }
                        }
                    }
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Char(value) => {
                if value == 'k'
                    && (self.active_block == ActiveBlock::Collections
                        || self.active_block == ActiveBlock::Examples
                        || self.active_block == ActiveBlock::Response
                        || (self.active_block == ActiveBlock::Params
                            && self.param_edit_mode == ParamEditMode::None
                            && self.active_request_tab != RequestTab::Body))
                {
                    if self.active_block == ActiveBlock::Response {
                        self.scroll_response_up(1);
                    } else if self.active_block == ActiveBlock::Collections {
                        self.select_previous_operation(project);
                    } else if self.active_block == ActiveBlock::Params {
                        self.selected_request_row = self.selected_request_row.saturating_sub(1);
                    } else if self.active_block == ActiveBlock::Examples {
                        self.selected_example_row = self.selected_example_row.saturating_sub(1);
                    }
                    return Ok(AppAction::Continue);
                }
                if value == 'j'
                    && (self.active_block == ActiveBlock::Collections
                        || self.active_block == ActiveBlock::Examples
                        || self.active_block == ActiveBlock::Response
                        || (self.active_block == ActiveBlock::Params
                            && self.param_edit_mode == ParamEditMode::None
                            && self.active_request_tab != RequestTab::Body))
                {
                    if self.active_block == ActiveBlock::Response {
                        self.scroll_response_down(1);
                    } else if self.active_block == ActiveBlock::Collections {
                        self.select_next_operation(project);
                    } else if self.active_block == ActiveBlock::Params {
                        self.selected_request_row = self.selected_request_row.saturating_add(1);
                    } else if self.active_block == ActiveBlock::Examples {
                        let max = self.model.examples.len().saturating_sub(1);
                        self.selected_example_row =
                            self.selected_example_row.saturating_add(1).min(max);
                    }
                    return Ok(AppAction::Continue);
                }
                if value == ' '
                    && self.active_block == ActiveBlock::Params
                    && self.active_request_tab != RequestTab::Body
                    && self.param_edit_mode == ParamEditMode::None
                {
                    let map = if self.active_request_tab == RequestTab::Query {
                        &mut self.draft.params
                    } else {
                        &mut self.draft.headers
                    };
                    if let Some(param) = map.get_mut(self.selected_request_row) {
                        if !param.required {
                            param.enabled = !param.enabled;
                        }
                    }
                    return Ok(AppAction::Continue);
                }

                if self.active_block == ActiveBlock::Request {
                    insert_char_at(&mut self.draft.url, self.text_cursor, value);
                    self.text_cursor = self.text_cursor.saturating_add(1);
                    self.handle_url_edited(project);
                } else if self.active_block == ActiveBlock::Params {
                    if self.active_request_tab == RequestTab::Body {
                        insert_char_at(&mut self.draft.body, self.text_cursor, value);
                        self.text_cursor = self.text_cursor.saturating_add(1);
                    } else if self.param_edit_mode != ParamEditMode::None {
                        let map = if self.active_request_tab == RequestTab::Query {
                            &mut self.draft.params
                        } else {
                            &mut self.draft.headers
                        };
                        if let Some(param) = map.get_mut(self.selected_request_row) {
                            if self.param_edit_mode == ParamEditMode::Key && !param.required {
                                insert_char_at(&mut param.key, self.text_cursor, value);
                                self.text_cursor = self.text_cursor.saturating_add(1);
                            } else if self.param_edit_mode == ParamEditMode::Value {
                                insert_char_at(&mut param.value, self.text_cursor, value);
                                self.text_cursor = self.text_cursor.saturating_add(1);
                            }
                        }
                    }
                }
                Ok(AppAction::Continue)
            }
            _ => Ok(AppAction::Continue),
        }
    }

    fn load_example(&mut self, example: &crate::project::ExampleFile) {
        if let Ok(data) = example.load_data() {
            if let Some(params) = data.params {
                for (k, v) in params {
                    if let Some(p) = self.draft.params.iter_mut().find(|p| p.key == k) {
                        p.value = v;
                        p.enabled = true;
                    } else {
                        self.draft.params.push(ParamState {
                            key: k,
                            value: v,
                            enabled: true,
                            required: false,
                        });
                    }
                }
            }
            if let Some(headers) = data.headers {
                for (k, v) in headers {
                    if let Some(p) = self.draft.headers.iter_mut().find(|p| p.key == k) {
                        p.value = v;
                        p.enabled = true;
                    } else {
                        self.draft.headers.push(ParamState {
                            key: k,
                            value: v,
                            enabled: true,
                            required: false,
                        });
                    }
                }
            }
            if let Some(body) = data.body {
                self.draft.body = serde_json::to_string_pretty(&body).unwrap_or_default();
            } else {
                self.draft.body.clear();
            }

            self.update_request_tab();
        }
    }

    fn update_request_tab(&mut self) {
        let query_empty = self.draft.params.iter().all(|p| !p.enabled || p.value.is_empty());
        let body_empty = self.draft.body.trim().is_empty();

        if query_empty && !body_empty {
            self.active_request_tab = RequestTab::Body;
        } else {
            self.active_request_tab = RequestTab::Query;
        }
    }

    fn select_operation(&mut self, operation: &crate::project::Operation, project: &RataProject) {
        self.selected_operation = Some((operation.method, operation.path.clone()));
        self.draft.method = operation.method;
        self.draft.url = format!("{{{{baseUrl}}}}{}", operation.path);
        self.draft.body.clear();
        self.draft.params.clear();
        self.draft.headers.clear();

        let examples_res = project.examples_for(operation);
        let examples = examples_res
            .as_ref()
            .map(|x| x.as_slice())
            .unwrap_or_default();
        self.model.examples = examples.iter().map(|e| e.name.clone()).collect();

        for param in &operation.parameters {
            let p = ParamState {
                key: param.name.clone(),
                value: String::new(),
                enabled: param.required,
                required: param.required,
            };
            if param.location == "header" {
                self.draft.headers.push(p);
            } else {
                self.draft.params.push(p);
            }
        }

        self.draft.headers.push(ParamState {
            key: "user-agent".to_string(),
            value: format!("rata/{}", env!("CARGO_PKG_VERSION")),
            enabled: true,
            required: false,
        });

        for (k, v) in project.global_headers() {
            self.draft.headers.push(ParamState {
                key: k,
                value: v,
                enabled: true,
                required: false,
            });
        }

        if let Some(first_example) = examples.first() {
            self.load_example(first_example);
        } else {
            self.update_request_tab();
        }
    }

    fn handle_url_edited(&mut self, project: Option<&RataProject>) {
        self.draft.params.clear();
        self.draft.headers.clear();
        self.draft.body.clear();
        self.model.examples.clear();
        self.selected_operation = None;
        self.examples_dropdown_open = false;

        self.draft.headers.push(ParamState {
            key: "user-agent".to_string(),
            value: format!("rata/{}", env!("CARGO_PKG_VERSION")),
            enabled: true,
            required: false,
        });

        if let Some(project) = project {
            for (k, v) in project.global_headers() {
                self.draft.headers.push(ParamState {
                    key: k,
                    value: v,
                    enabled: true,
                    required: false,
                });
            }
        }

        let Some(project) = project else { return };

        let resolved_url = crate::template::render(&self.draft.url, &project.variables());
        if let Ok(Some(matched)) = project.match_url(self.draft.method, &resolved_url) {
            self.selected_operation =
                Some((matched.operation.method, matched.operation.path.clone()));

            let examples_res = project.examples_for(&matched.operation);
            let examples = examples_res
                .as_ref()
                .map(|x| x.as_slice())
                .unwrap_or_default();
            self.model.examples = examples.iter().map(|e| e.name.clone()).collect();

            for param in &matched.operation.parameters {
                let p = ParamState {
                    key: param.name.clone(),
                    value: String::new(),
                    enabled: param.required,
                    required: param.required,
                };
                if param.location == "header" {
                    self.draft.headers.push(p);
                } else {
                    self.draft.params.push(p);
                }
            }

            if let Some(first_example) = examples.first() {
                self.load_example(first_example);
            }
        }
    }

    fn get_visible_operations<'a>(
        &self,
        project: &'a RataProject,
    ) -> Vec<&'a crate::project::Operation> {
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
            if ops.is_empty() {
                return;
            }
            let mut next_op = ops[0];
            if let Some(selected) = &self.selected_operation {
                if let Some(pos) = ops
                    .iter()
                    .position(|op| op.method == selected.0 && op.path == selected.1)
                {
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
            if ops.is_empty() {
                return;
            }
            let mut prev_op = ops[0];
            if let Some(selected) = &self.selected_operation {
                if let Some(pos) = ops
                    .iter()
                    .position(|op| op.method == selected.0 && op.path == selected.1)
                {
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

        if let Some(error) = self.error_popup.clone() {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if !contains(self.error_popup_area, mouse.column, mouse.row) {
                        self.error_popup = None;
                        self.text_selection = None;
                    } else {
                        self.drag_target = DragTarget::ErrorPopupSelection;
                        let inner_y = self.error_popup_area.y + 1;
                        let inner_x = self.error_popup_area.x + 1;
                        let v_line = mouse.row.saturating_sub(inner_y) as usize;
                        let v_col = mouse.column.saturating_sub(inner_x) as usize;
                        let width = self.error_popup_area.width.saturating_sub(2) as usize;

                        let char_idx = visual_to_char_index(&error, v_line, v_col, width, true);
                        let logical_pos = char_index_to_logical(&error, char_idx);

                        self.text_selection = Some(Selection {
                            start: logical_pos,
                            end: logical_pos,
                        });
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if self.drag_target == DragTarget::ErrorPopupSelection {
                        if let Some(sel) = &mut self.text_selection {
                            let inner_y = self.error_popup_area.y + 1;
                            let inner_x = self.error_popup_area.x + 1;
                            let v_line = mouse.row.saturating_sub(inner_y) as usize;
                            let v_col = mouse.column.saturating_sub(inner_x) as usize;
                            let width = self.error_popup_area.width.saturating_sub(2) as usize;

                            let char_idx = visual_to_char_index(&error, v_line, v_col, width, true);
                            sel.end = char_index_to_logical(&error, char_idx);
                        }
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if self.drag_target == DragTarget::ErrorPopupSelection {
                        if let Some(sel) = self.text_selection {
                            let text = Self::extract_text_selection(&error, sel);
                            if !text.is_empty() {
                                if let Some(clipboard) = &mut self.clipboard {
                                    let _ = clipboard.set_text(text);
                                }
                            }
                        }
                        self.drag_target = DragTarget::None;
                    }
                }
                _ => {}
            }
            return;
        }

        if matches!(mouse.kind, MouseEventKind::ScrollUp) {
            if contains(self.response_area, mouse.column, mouse.row) {
                self.active_block = ActiveBlock::Response;
                self.scroll_response_up(3);
            } else if contains(self.params_area, mouse.column, mouse.row) {
                self.active_block = ActiveBlock::Params;
                self.text_cursor = usize::MAX;
                self.scroll_request_up(3);
            }
            return;
        }

        if matches!(mouse.kind, MouseEventKind::ScrollDown) {
            if contains(self.response_area, mouse.column, mouse.row) {
                self.active_block = ActiveBlock::Response;
                self.scroll_response_down(3);
            } else if contains(self.params_area, mouse.column, mouse.row) {
                self.active_block = ActiveBlock::Params;
                self.text_cursor = usize::MAX;
                self.scroll_request_down(3);
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
                        self.request_height =
                            mouse.row.saturating_sub(self.request_area.y).max(3).min(20);
                    }
                    DragTarget::Response => {
                        let main_rest_y = self.params_area.y;
                        let main_rest_h = self
                            .params_area
                            .height
                            .saturating_add(self.response_area.height);
                        if main_rest_h > 0 {
                            let offset = mouse.row.saturating_sub(main_rest_y);
                            let percent = (offset as u32 * 100 / main_rest_h as u32) as u16;
                            self.response_height_percent =
                                100u16.saturating_sub(percent).max(10).min(90);
                        }
                    }
                    DragTarget::ScrollResponse => {
                        if let Some(last) = self.drag_last_row {
                            if mouse.row > last {
                                self.scroll_response_down(mouse.row - last);
                            } else if mouse.row < last {
                                self.scroll_response_up(last - mouse.row);
                            }
                        }
                        self.drag_last_row = Some(mouse.row);
                    }
                    DragTarget::ScrollRequest => {
                        if let Some(last) = self.drag_last_row {
                            if mouse.row > last {
                                self.scroll_request_down(mouse.row - last);
                            } else if mouse.row < last {
                                self.scroll_request_up(last - mouse.row);
                            }
                        }
                        self.drag_last_row = Some(mouse.row);
                    }
                    DragTarget::ResponseSelection => {
                        let inner_y = self.response_area.y + 1;
                        let inner_x = self.response_area.x + 1;
                        let inner_bottom =
                            self.response_area.y + self.response_area.height.saturating_sub(1);

                        if mouse.row < inner_y {
                            self.scroll_response_up(inner_y.saturating_sub(mouse.row));
                        } else if mouse.row >= inner_bottom {
                            self.scroll_response_down(mouse.row.saturating_sub(inner_bottom) + 1);
                        }

                        let text_str = self.active_response_string();
                        let width = self.response_area.width.saturating_sub(2) as usize;
                        let wrap = self.active_response_tab == ResponseTab::Body && self.wrap_body;

                        if let Some(sel) = &mut self.text_selection {
                            let v_line = if mouse.row < inner_y {
                                self.response_scroll as usize
                            } else {
                                mouse.row.saturating_sub(inner_y) as usize
                                    + self.response_scroll as usize
                            };
                            let v_col = mouse.column.saturating_sub(inner_x) as usize;

                            let char_idx =
                                visual_to_char_index(&text_str, v_line, v_col, width, wrap);
                            sel.end = char_index_to_logical(&text_str, char_idx);
                            self.text_cursor = char_idx;
                        }
                    }
                    DragTarget::RequestSelection => {
                        let inner_y = self.params_area.y + 1;
                        let inner_x = self.params_area.x + 1;
                        let inner_bottom =
                            self.params_area.y + self.params_area.height.saturating_sub(1);

                        if mouse.row < inner_y {
                            self.scroll_request_up(inner_y.saturating_sub(mouse.row));
                        } else if mouse.row >= inner_bottom {
                            self.scroll_request_down(mouse.row.saturating_sub(inner_bottom) + 1);
                        }

                        let text_str = self.draft.body.clone();
                        let width = self.params_area.width.saturating_sub(2) as usize;
                        let wrap = self.wrap_body;

                        if let Some(sel) = &mut self.request_selection {
                            let v_line = if mouse.row < inner_y {
                                self.request_scroll as usize
                            } else {
                                mouse.row.saturating_sub(inner_y) as usize
                                    + self.request_scroll as usize
                            };
                            let v_col = mouse.column.saturating_sub(inner_x) as usize;

                            let char_idx =
                                visual_to_char_index(&text_str, v_line, v_col, width, wrap);
                            sel.end = char_index_to_logical(&text_str, char_idx);
                            self.text_cursor = char_idx;
                            self.ensure_cursor_visible();
                        }
                    }
                    DragTarget::None | DragTarget::ErrorPopupSelection => {}
                }
                return;
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.drag_target == DragTarget::ResponseSelection {
                    if let Some(sel) = self.text_selection {
                        let text = self.extract_selection(sel);
                        if !text.is_empty() {
                            if let Some(clipboard) = &mut self.clipboard {
                                let _ = clipboard.set_text(text);
                            }
                        }
                    }
                } else if self.drag_target == DragTarget::RequestSelection {
                    if let Some(sel) = self.request_selection {
                        let text = self.extract_request_selection(sel);
                        if !text.is_empty() {
                            if let Some(clipboard) = &mut self.clipboard {
                                let _ = clipboard.set_text(text);
                            }
                        }
                    }
                }
                self.drag_target = DragTarget::None;
                return;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let method_str_len =
                    format!(" {} ▾ ", self.draft.method.label()).chars().count() as u16;
                let clicked_method_dropdown = mouse.row == self.request_area.y + 1
                    && mouse.column >= self.request_area.x + 1
                    && mouse.column < self.request_area.x + 1 + method_str_len;
                let clicked_inside_method_dropdown = self.method_dropdown_open
                    && contains(self.method_dropdown_area, mouse.column, mouse.row);

                if self.method_dropdown_open
                    && !clicked_method_dropdown
                    && !clicked_inside_method_dropdown
                {
                    self.method_dropdown_open = false;
                }

                if clicked_method_dropdown {
                    self.method_dropdown_open = !self.method_dropdown_open;
                    if self.method_dropdown_open {
                        self.active_block = ActiveBlock::MethodDropdown;
                        self.selected_method_row = METHODS
                            .iter()
                            .position(|m| m == &self.draft.method)
                            .unwrap_or(0);
                    } else {
                        self.active_block = ActiveBlock::Request;
                        self.text_cursor = usize::MAX;
                    }
                    return;
                }

                if clicked_inside_method_dropdown {
                    self.active_block = ActiveBlock::Request;
                    self.method_dropdown_open = false; // Close when clicked inside
                    let clicked_row = mouse.row.saturating_sub(self.method_dropdown_area.y + 1);
                    if let Some(m) = METHODS.get(clicked_row as usize) {
                        self.draft.method = *m;
                        self.handle_url_edited(project);
                    }
                    return;
                }

                let dropdown_x = self.request_area.right().saturating_sub(14);
                let clicked_dropdown_toggle = mouse.row == self.request_area.y
                    && mouse.column >= dropdown_x
                    && mouse.column < self.request_area.right();
                let clicked_inside_dropdown = self.examples_dropdown_open
                    && contains(self.examples_area, mouse.column, mouse.row);

                if self.examples_dropdown_open
                    && !clicked_dropdown_toggle
                    && !clicked_inside_dropdown
                {
                    self.examples_dropdown_open = false;
                }

                if clicked_dropdown_toggle {
                    self.examples_dropdown_open = !self.examples_dropdown_open;
                    if self.examples_dropdown_open {
                        self.active_block = ActiveBlock::Examples;
                    } else {
                        self.active_block = ActiveBlock::Request;
                        self.text_cursor = usize::MAX;
                    }
                    return;
                }

                if clicked_inside_dropdown {
                    self.active_block = ActiveBlock::Request;
                    self.examples_dropdown_open = false; // Close when clicked inside
                    let clicked_row = mouse.row.saturating_sub(self.examples_area.y + 1);
                    if let Some(example_name) = self.model.examples.get(clicked_row as usize) {
                        if let Some(project) = project {
                            if let Some(selected) = &self.selected_operation {
                                if let Some(op) = project
                                    .collections()
                                    .iter()
                                    .flat_map(|c| &c.operations)
                                    .find(|o| o.method == selected.0 && o.path == selected.1)
                                {
                                    if let Some(example_file) = project
                                        .examples_for(op)
                                        .ok()
                                        .unwrap_or_default()
                                        .iter()
                                        .find(|e| &e.name == example_name)
                                    {
                                        self.load_example(example_file);
                                    }
                                }
                            }
                        }
                    }
                    return;
                }

                if let Some(tab) =
                    response_tab_at(self, mouse.column, mouse.row, self.response_tab_origin)
                {
                    self.active_response_tab = tab;
                    self.response_scroll = 0;
                    self.active_block = ActiveBlock::Response;
                    self.text_selection = None;
                    return;
                }

                if let Some(tab) = request_tab_at(self, mouse.column, mouse.row) {
                    self.active_request_tab = tab;
                    self.active_block = ActiveBlock::Params;
                    self.text_cursor = usize::MAX;
                    self.selected_request_row = 0;
                    return;
                }

                if mouse.column == self.collections_area.right().saturating_sub(1)
                    || mouse.column == self.collections_area.right()
                {
                    self.active_block = ActiveBlock::Collections;
                    self.drag_target = DragTarget::Collections;
                    return;
                }

                if contains(self.request_area, mouse.column, mouse.row) {
                    if contains(self.send_button_area, mouse.column, mouse.row) {
                        self.send(project);
                        return;
                    }
                    self.active_block = ActiveBlock::Request;
                    let method_icon = if self.method_dropdown_open {
                        "▴"
                    } else {
                        "▾"
                    };
                    let method_str_len = format!(" {} {} ", self.draft.method.label(), method_icon)
                        .chars()
                        .count() as u16;
                    let url_start_x = self.request_area.x + 1 + method_str_len + 2;
                    if mouse.column >= url_start_x {
                        let clicked_col = (mouse.column - url_start_x) as usize;
                        self.text_cursor = clicked_col.min(self.draft.url.chars().count());
                    } else {
                        self.text_cursor = usize::MAX;
                    }
                    return;
                }

                if mouse.column >= self.request_area.x {
                    if mouse.row == self.params_area.y.saturating_sub(1)
                        || mouse.row == self.params_area.y
                    {
                        self.active_block = ActiveBlock::Params;
                        self.text_cursor = usize::MAX;
                        self.drag_target = DragTarget::Request;
                        return;
                    }
                    if mouse.row == self.response_area.y.saturating_sub(1)
                        || mouse.row == self.response_area.y
                    {
                        self.active_block = ActiveBlock::Response;
                        self.drag_target = DragTarget::Response;
                        return;
                    }
                }

                if contains(self.response_area, mouse.column, mouse.row) {
                    self.active_block = ActiveBlock::Response;
                    self.drag_target = DragTarget::ResponseSelection;
                    let inner_y = self.response_area.y + 1;
                    let inner_x = self.response_area.x + 1;
                    let v_line =
                        mouse.row.saturating_sub(inner_y) as usize + self.response_scroll as usize;
                    let v_col = mouse.column.saturating_sub(inner_x) as usize;

                    let text_str = self.active_response_string();
                    let width = self.response_area.width.saturating_sub(2) as usize;
                    let wrap = self.active_response_tab == ResponseTab::Body && self.wrap_body;
                    let char_idx = visual_to_char_index(&text_str, v_line, v_col, width, wrap);
                    let logical_pos = char_index_to_logical(&text_str, char_idx);

                    self.text_selection = Some(Selection {
                        start: logical_pos,
                        end: logical_pos,
                    });
                    self.drag_last_row = Some(mouse.row);
                    return;
                } else if contains(self.params_area, mouse.column, mouse.row) {
                    self.active_block = ActiveBlock::Params;
                    if self.active_request_tab == RequestTab::Body {
                        self.drag_target = DragTarget::RequestSelection;
                        let inner_y = self.params_area.y + 1;
                        let inner_x = self.params_area.x + 1;
                        let v_line = mouse.row.saturating_sub(inner_y) as usize
                            + self.request_scroll as usize;
                        let v_col = mouse.column.saturating_sub(inner_x) as usize;

                        let width = self.params_area.width.saturating_sub(2) as usize;
                        let wrap = self.wrap_body;
                        let char_idx =
                            visual_to_char_index(&self.draft.body, v_line, v_col, width, wrap);
                        let logical_pos = char_index_to_logical(&self.draft.body, char_idx);

                        self.request_selection = Some(Selection {
                            start: logical_pos,
                            end: logical_pos,
                        });
                        self.text_cursor = char_idx;
                    } else {
                        self.drag_target = DragTarget::ScrollRequest;
                        self.text_cursor = usize::MAX;
                    }
                    self.drag_last_row = Some(mouse.row);
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
        }
    }

    fn set_response_tabs_area(&mut self, area: Rect) {
        self.response_tab_origin = (area.x, area.y);
    }

    pub fn response_tabs(&self) -> [String; 3] {
        [
            "Body".to_string(),
            format!("Headers ({})", self.response.headers.len()),
            format!("Cookies ({})", self.response.cookies.len()),
        ]
    }

    pub fn request_tabs(&self) -> [String; 3] {
        [
            format!(
                " Query ({}) ",
                self.draft.params.iter().filter(|p| p.enabled).count()
            ),
            " Body ".to_string(),
            format!(
                " Headers ({}) ",
                self.draft.headers.iter().filter(|p| p.enabled).count()
            ),
        ]
    }

    pub fn response_tab_bounds(&self) -> [(u16, u16); 3] {
        let tabs = self.response_tabs();
        let mut bounds = [(0, 0); 3];
        let mut current = 1;
        for i in 0..3 {
            let start = current;
            let end = start + tabs[i].chars().count() as u16 - 1;
            bounds[i] = (start, end);
            current = end + 1 + 3;
        }
        bounds
    }

    pub fn active_response_string(&self) -> String {
        if let Some(error) = &self.response.error {
            return error.clone();
        }
        match self.active_response_tab {
            ResponseTab::Body => crate::components::body::pretty_body(&self.response.body),
            ResponseTab::Headers => format_pairs(&self.response.headers, "No headers"),
            ResponseTab::Cookies => {
                if self.response.cookies.is_empty() {
                    "No cookies".to_string()
                } else {
                    self.response.cookies.join("\n")
                }
            }
        }
    }

    fn extract_text_selection(text: &str, sel: Selection) -> String {
        let (mut start, mut end) = (sel.start, sel.end);
        if start.0 > end.0 || (start.0 == end.0 && start.1 > end.1) {
            std::mem::swap(&mut start, &mut end);
        }
        let mut result = String::new();
        for (line_idx, line) in text.lines().enumerate() {
            if line_idx >= start.0 && line_idx <= end.0 {
                let sel_start_col = if line_idx == start.0 { start.1 } else { 0 };
                let sel_end_col = if line_idx == end.0 { end.1 } else { usize::MAX };

                for (col, c) in line.chars().enumerate() {
                    if col >= sel_start_col && col <= sel_end_col {
                        result.push(c);
                    }
                }
                if line_idx < end.0 {
                    result.push('\n');
                }
            }
        }
        result
    }

    pub fn extract_selection(&self, sel: Selection) -> String {
        Self::extract_text_selection(&self.active_response_string(), sel)
    }

    pub fn extract_request_selection(&self, sel: Selection) -> String {
        Self::extract_text_selection(&self.draft.body, sel)
    }
}

fn execute_request(
    draft: &RequestDraft,
    project: Option<&RataProject>,
) -> anyhow::Result<ResponseView> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("") // Empty default so it can be disabled
        .build()
        .unwrap();

    let mut variables = std::collections::HashMap::new();
    if let Some(project) = project {
        variables = project.variables();
        if !variables.contains_key("baseUrl") {
            if let Some(server) = project.server_url() {
                variables.insert(
                    "baseUrl".to_string(),
                    server.trim_end_matches('/').to_string(),
                );
            }
        }
    }

    let mut final_url = draft.url.clone();
    let mut query_params = Vec::new();

    for param in &draft.params {
        if !param.enabled || param.key.is_empty() {
            continue;
        }
        let p1 = format!("{{{{{}}}}}", param.key);
        let p2 = format!("{{{}}}", param.key);
        if final_url.contains(&p1) || final_url.contains(&p2) {
            final_url = final_url.replace(&p1, &param.value);
            final_url = final_url.replace(&p2, &param.value);
        } else if !param.value.is_empty() {
            query_params.push((&param.key, &param.value));
        }
    }

    final_url = crate::render(&final_url, &variables);

    let mut request = client.request(draft.method.reqwest(), &final_url);

    if !query_params.is_empty() {
        request = request.query(&query_params);
    }

    for param in &draft.headers {
        if !param.enabled || param.key.is_empty() {
            continue;
        }
        let final_value = crate::render(&param.value, &variables);
        request = request.header(&param.key, &final_value);
    }

    let final_body = crate::render(&draft.body, &variables);

    let mut response = request.body(final_body).send()?;
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

fn request_tab_at(app: &TuiApp, column: u16, row: u16) -> Option<RequestTab> {
    if row != app.params_area.y {
        return None;
    }
    let origin_column = app.params_area.x + 1;
    if column < origin_column {
        return None;
    }
    let offset = column - origin_column;
    let tabs = app.request_tabs();
    let mut current = 0;
    for (i, tab) in tabs.iter().enumerate() {
        let start = current;
        let end = start + tab.chars().count() as u16 - 1;
        if offset >= start && offset <= end {
            return match i {
                0 => Some(RequestTab::Query),
                1 => Some(RequestTab::Body),
                2 => Some(RequestTab::Headers),
                _ => None,
            };
        }
        current = end + 1 + 1; // +1 for the separator "·"
    }
    None
}

fn response_tab_at(app: &TuiApp, column: u16, row: u16, origin: (u16, u16)) -> Option<ResponseTab> {
    let (origin_column, origin_row) = origin;
    if row != origin_row || column < origin_column {
        return None;
    }

    let offset = column - origin_column;
    let bounds = app.response_tab_bounds();

    if offset >= bounds[0].0 && offset <= bounds[0].1 {
        return Some(ResponseTab::Body);
    }
    if offset >= bounds[1].0 && offset <= bounds[1].1 {
        return Some(ResponseTab::Headers);
    }
    if offset >= bounds[2].0 && offset <= bounds[2].1 {
        return Some(ResponseTab::Cookies);
    }
    None
}

const RESPONSE_TAB_ROW: u16 = 3;

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
    Quit,
    Continue,
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
        .map(|operation| format!("{{{{baseUrl}}}}{}", operation.path))
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

pub fn run(mut project: Option<RataProject>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new(project.as_ref());
    let result = run_loop(&mut terminal, &mut project, &mut app);

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
    project: &mut Option<RataProject>,
    app: &mut TuiApp,
) -> anyhow::Result<()> {
    let mut last_mouse_pos: Option<(u16, u16)> = None;
    let mut last_modified = project.as_ref()
        .map(|p| p.openapi_path().to_path_buf())
        .and_then(|p| std::fs::metadata(&p).ok())
        .and_then(|m| m.modified().ok());

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let app_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(area);

            let url_and_rest = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(app.request_height), Constraint::Min(0)])
                .split(app_layout[0]);

            let url_area = url_and_rest[0];
            let rest_area = url_and_rest[1];

            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(app.collections_width),
                    Constraint::Min(0),
                ])
                .split(rest_area);
            app.collections_area = body[0];
            let main_rest = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(100_u16.saturating_sub(app.response_height_percent)),
                    Constraint::Percentage(app.response_height_percent),
                ])
                .split(body[1]);
            let request_body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0)])
                .split(main_rest[0]);

            app.request_area = url_area;
            app.params_area = request_body[0];
            app.response_area = main_rest[1];

            let send_len = 11;
            app.send_button_area = Rect {
                x: url_area.right().saturating_sub(1 + send_len),
                y: url_area.y + 1,
                width: send_len,
                height: 1,
            };

            frame.render_widget(request_line(app), url_area);
            frame.render_widget(collections(project.as_ref(), app), body[0]);
            render_request_block(frame, app, project.as_ref(), request_body[0]);
            render_response(frame, app, main_rest[1]);
            frame.render_widget(render_shortcut_bar(app), app_layout[1]);

            if app.examples_dropdown_open {
                let dropdown_width = 30;
                let dropdown_height = app.model.examples.len().max(1).min(10) as u16 + 2;
                let area = Rect {
                    x: app.request_area.right().saturating_sub(dropdown_width + 1),
                    y: app.request_area.y + 1,
                    width: dropdown_width,
                    height: dropdown_height,
                };
                app.examples_area = area;
                frame.render_widget(ratatui::widgets::Clear, area);
                frame.render_widget(examples(project.as_ref(), app), area);
            } else {
                app.examples_area = Rect::default();
            }

            if app.method_dropdown_open {
                let dropdown_width = 15;
                let dropdown_height = METHODS.len() as u16 + 2;
                let area = Rect {
                    x: app.request_area.x + 1,
                    y: app.request_area.y + 1,
                    width: dropdown_width,
                    height: dropdown_height,
                };
                app.method_dropdown_area = area;
                frame.render_widget(ratatui::widgets::Clear, area);
                frame.render_widget(method_dropdown(app), area);
            } else {
                app.method_dropdown_area = Rect::default();
            }

            if let Some(error) = &app.error_popup {
                let area = frame.area();
                let popup_width = (area.width * 6) / 10;
                let popup_height = (area.height * 6) / 10;
                let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let y = area.y + (area.height.saturating_sub(popup_height)) / 2;

                app.error_popup_area = Rect {
                    x,
                    y,
                    width: popup_width.max(20),
                    height: popup_height.max(10),
                };
                render_error_popup(frame, error, app.error_popup_area, app.text_selection);
            } else {
                app.error_popup_area = Rect::default();
            }
        })?;

        if let Some(rx) = &app.send_rx {
            if let Ok(res) = rx.try_recv() {
                match res {
                    Ok(response) => {
                        app.response = response;
                        if let Some(proj) = project.as_ref() {
                            if let Some((method, path)) = &app.selected_operation {
                                if let Some(status) = app.response.status {
                                    match proj.validate_response_body(*method, path, status, &app.response.body) {
                                        Ok(errors) => {
                                            if !errors.is_empty() {
                                                app.error_popup = Some(format!("Response Validation Failed:\n{}", errors.join("\n")));
                                            }
                                        }
                                        Err(e) => {
                                            app.error_popup = Some(format!("Response Schema Error:\n{}", e));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => app.response.error = Some(error.to_string()),
                }
                app.is_sending = false;
                app.send_rx = None;
            } else {
                app.sending_frame = app.sending_frame.wrapping_add(1);
            }
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => match app.handle_key(key, project.as_ref())? {
                    AppAction::Quit => return Ok(()),
                    AppAction::Continue => {}
                },
                Event::Mouse(mouse) => {
                    last_mouse_pos = Some((mouse.column, mouse.row));
                    app.handle_mouse(mouse, project.as_ref());
                }
                _ => {}
            }
        } else {
            if let Some(p) = project.as_ref() {
                if let Ok(m) = std::fs::metadata(p.openapi_path()) {
                    if let Ok(current_modified) = m.modified() {
                        if Some(current_modified) != last_modified {
                            if let Ok(Some(new_project)) = crate::RataProject::discover(std::env::current_dir().unwrap_or_default()) {
                                *project = Some(new_project);
                            }
                            last_modified = Some(current_modified);
                        }
                    }
                }
            }
            if app.drag_target == DragTarget::ResponseSelection {
                if let Some((col, row)) = last_mouse_pos {
                    let synthetic_mouse = crossterm::event::MouseEvent {
                        kind: crossterm::event::MouseEventKind::Drag(
                            crossterm::event::MouseButton::Left,
                        ),
                        column: col,
                        row,
                        modifiers: crossterm::event::KeyModifiers::empty(),
                    };
                    app.handle_mouse(synthetic_mouse, project.as_ref());
                }
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
                    let is_selected = app.selected_operation.as_ref()
                        == Some(&(operation.method, operation.path.clone()));
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
                .title(Span::styled(
                    " Collections ",
                    Style::default().fg(Color::White),
                ))
                .borders(Borders::ALL)
                .style(Style::default().bg(PANEL).fg(TEXT))
                .border_style(border_style),
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

    let border_style = if app.active_block == ActiveBlock::Request {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    let example_title = if app.examples_dropdown_open {
        " Examples ▴ "
    } else {
        " Examples ▾ "
    };

    let method_icon = if app.method_dropdown_open {
        "▴"
    } else {
        "▾"
    };

    let mut spans = vec![
        Span::styled(
            format!(" {} {} ", app.draft.method.label(), method_icon),
            method_style(app.draft.method)
                .bg(SELECTED_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(" ", Style::default().fg(TEXT).bg(SELECTED_BG)),
    ];
    if app.active_block == ActiveBlock::Request {
        spans.extend(render_with_cursor_spans(
            &url,
            if app.cursor_visible() { Some(app.text_cursor) } else { None },
            Style::default().fg(TEXT).bg(SELECTED_BG),
        ));
    } else {
        spans.push(Span::styled(
            url.clone(),
            Style::default().fg(TEXT).bg(SELECTED_BG),
        ));
    }
    spans.push(Span::styled(" ", Style::default().fg(TEXT).bg(SELECTED_BG)));

    let method_str_len = format!(" {} {} ", app.draft.method.label(), method_icon)
        .chars()
        .count();
    let occupied = method_str_len + 1 + 1 + url.chars().count() + 1;
    let remaining = app
        .request_area
        .width
        .saturating_sub(2)
        .saturating_sub(occupied as u16);

    let (send_len, mut send_spans) = if app.is_sending {
        let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame = frames[app.sending_frame % frames.len()];
        let btn = vec![
            Span::styled(
                format!("{}", frame),
                Style::default().fg(Color::Green).bg(SELECTED_BG),
            ),
            Span::styled(
                "  SEND  ",
                Style::default()
                    .fg(Color::Yellow)
                    .bg(SELECTED_BG)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        (11, btn)
    } else {
        let btn = vec![
            Span::styled(" ", Style::default().bg(SELECTED_BG)),
            Span::styled(
                "  SEND  ",
                Style::default()
                    .fg(Color::White)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        (11, btn)
    };

    if remaining >= send_len as u16 + 1 {
        let padding = remaining - send_len as u16;
        spans.push(Span::styled(
            " ".repeat(padding as usize),
            Style::default().bg(SELECTED_BG),
        ));
        spans.append(&mut send_spans);
    } else if remaining > 0 {
        spans.push(Span::styled(
            " ".repeat(remaining as usize),
            Style::default().bg(SELECTED_BG),
        ));
    }

    Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(Span::styled(" URL ", Style::default().fg(Color::White)))
            .title_top(Line::from(example_title).right_aligned())
            .borders(Borders::ALL)
            .style(Style::default().bg(PANEL).fg(TEXT))
            .border_style(border_style),
    )
}

fn render_shortcut_bar(_app: &TuiApp) -> Paragraph<'static> {
    let mut spans = Vec::new();
    let base_bg = PANEL;
    let bgs = [SELECTED_BG, BORDER];
    let mut bg_idx = 0;

    // Start with "Ctrl" block
    spans.push(Span::styled(
        " Ctrl ",
        Style::default()
            .fg(PANEL)
            .bg(ACCENT)
            .add_modifier(Modifier::BOLD),
    ));
    let mut current_bg = bgs[0];
    spans.push(Span::styled(
        "\u{E0B0} ",
        Style::default().fg(ACCENT).bg(current_bg),
    ));

    let mut shortcuts = vec![("q", "Quit"), ("s", "Send")];
    if _app.active_block == ActiveBlock::Params && _app.active_request_tab != RequestTab::Body {
        shortcuts.push(("e", "Edit"));
    }
    if _app.wrap_body {
        shortcuts.push(("w", "Unwrap"));
    } else {
        shortcuts.push(("w", "Wrap"));
    }

    for (i, (key, desc)) in shortcuts.iter().enumerate() {
        let is_last = i == shortcuts.len() - 1;
        spans.push(Span::styled(
            format!("{}", key),
            Style::default()
                .fg(GREEN)
                .bg(current_bg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {} ", desc.to_uppercase()),
            Style::default().fg(TEXT).bg(current_bg),
        ));

        let next_bg = if is_last {
            base_bg
        } else {
            bgs[(bg_idx + 1) % bgs.len()]
        };
        spans.push(Span::styled(
            "\u{E0B0} ",
            Style::default().fg(current_bg).bg(next_bg),
        ));

        if !is_last {
            bg_idx = (bg_idx + 1) % bgs.len();
            current_bg = next_bg;
        }
    }

    Paragraph::new(Line::from(spans)).style(Style::default().bg(base_bg))
}

fn render_request_block(
    frame: &mut ratatui::Frame,
    app: &TuiApp,
    _project: Option<&RataProject>,
    area: Rect,
) {
    let border_style = if app.active_block == ActiveBlock::Params {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    let request_tabs = app.request_tabs();
    let spans = vec![
        Span::styled(
            request_tabs[0].clone(),
            if app.active_request_tab == RequestTab::Query {
                accent_style().add_modifier(Modifier::BOLD)
            } else {
                muted_style()
            },
        ),
        Span::styled("·", muted_style()),
        Span::styled(
            request_tabs[1].clone(),
            if app.active_request_tab == RequestTab::Body {
                accent_style().add_modifier(Modifier::BOLD)
            } else {
                muted_style()
            },
        ),
        Span::styled("·", muted_style()),
        Span::styled(
            request_tabs[2].clone(),
            if app.active_request_tab == RequestTab::Headers {
                accent_style().add_modifier(Modifier::BOLD)
            } else {
                muted_style()
            },
        ),
    ];
    let tabs = Line::from(spans);

    let block = Block::default()
        .title_top(tabs)
        .title_top(
            Line::from(Span::styled(" Request ", Style::default().fg(Color::White)))
                .right_aligned(),
        )
        .borders(Borders::ALL)
        .style(Style::default().bg(PANEL).fg(TEXT))
        .border_style(border_style);

    match app.active_request_tab {
        RequestTab::Body => {
            let text = if app.draft.body.is_empty()
                && !(app.active_block == ActiveBlock::Params
                    && app.active_request_tab == RequestTab::Body)
            {
                "No request body".to_string()
            } else {
                app.draft.body.clone()
            };
            let cursor = if app.active_block == ActiveBlock::Params
                && app.active_request_tab == RequestTab::Body
                && app.cursor_visible()
            {
                Some(app.text_cursor)
            } else {
                None
            };

            crate::components::body::render_body_with_scrollbar(
                frame,
                area,
                &text,
                Some(block),
                app.request_scroll,
                app.wrap_body,
                app.request_selection,
                cursor,
            );
        }
        RequestTab::Query => {
            let mut rows = Vec::new();
            let params = &app.draft.params;
            for (i, param) in params.iter().enumerate() {
                let display_key = if app.active_block == ActiveBlock::Params
                    && app.param_edit_mode == ParamEditMode::Key
                    && i == app.selected_request_row
                {
                    Line::from(render_with_cursor_spans(
                        &param.key,
                        if app.cursor_visible() { Some(app.text_cursor) } else { None },
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
                        if app.cursor_visible() { Some(app.text_cursor) } else { None },
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
        RequestTab::Headers => {
            let mut rows = Vec::new();
            let params = &app.draft.headers;
            for (i, param) in params.iter().enumerate() {
                let display_key = if app.active_block == ActiveBlock::Params
                    && app.param_edit_mode == ParamEditMode::Key
                    && i == app.selected_request_row
                {
                    Line::from(render_with_cursor_spans(
                        &param.key,
                        if app.cursor_visible() { Some(app.text_cursor) } else { None },
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
                        if app.cursor_visible() { Some(app.text_cursor) } else { None },
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
            .header(
                Row::new(["", "Key", "Value"]).style(muted_style().add_modifier(Modifier::BOLD)),
            )
            .block(block)
            .style(Style::default().fg(TEXT));
            frame.render_widget(t, area);
        }
    }
}

fn examples(_project: Option<&RataProject>, app: &TuiApp) -> List<'static> {
    let border_style = if app.active_block == ActiveBlock::Examples {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    let items = if app.model.examples.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No examples",
            muted_style(),
        )))]
    } else {
        app.model
            .examples
            .iter()
            .enumerate()
            .map(|(i, example)| {
                let mut row = ListItem::new(Line::from(vec![
                    Span::styled("• ", accent_style()),
                    Span::styled(example.clone(), Style::default().fg(TEXT)),
                ]));
                if app.active_block == ActiveBlock::Examples && i == app.selected_example_row {
                    row = row.style(Style::default().bg(SELECTED_BG));
                }
                row
            })
            .collect()
    };

    List::new(items)
        .block(
            Block::default()
                .title(" Examples ")
                .borders(Borders::ALL)
                .style(Style::default().bg(PANEL).fg(TEXT))
                .border_style(border_style),
        )
        .style(Style::default().fg(TEXT))
}

fn method_dropdown(app: &TuiApp) -> List<'static> {
    let border_style = if app.active_block == ActiveBlock::MethodDropdown {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    };

    let items: Vec<ListItem> = METHODS
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut style = method_style(*m);
            if app.active_block == ActiveBlock::MethodDropdown && i == app.selected_method_row {
                style = style.bg(SELECTED_BG);
            }
            ListItem::new(Span::styled(format!(" {:<6}", m.label()), style))
        })
        .collect();

    List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Method ")
            .style(Style::default().bg(PANEL)),
    )
}

fn render_response(frame: &mut ratatui::Frame<'_>, app: &mut TuiApp, area: Rect) {
    let block = response_block(app);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    app.set_response_tabs_area(Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    });

    let view_height = inner.height as usize;

    if app.active_response_tab == ResponseTab::Headers {
        let header_rows: Vec<ratatui::widgets::Row> = app
            .response
            .headers
            .iter()
            .map(|(k, v)| {
                ratatui::widgets::Row::new(vec![
                    ratatui::widgets::Cell::from(Span::styled(
                        k.clone(),
                        Style::default().fg(BLUE),
                    )),
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
    } else {
        let raw_string = app.active_response_string();

        if app.active_response_tab == ResponseTab::Body && app.response.error.is_none() {
            crate::components::body::render_body_with_scrollbar(
                frame,
                inner,
                &raw_string,
                None,
                app.response_scroll,
                app.wrap_body,
                app.text_selection,
                if app.active_block == ActiveBlock::Response && app.cursor_visible() {
                    Some(app.text_cursor)
                } else {
                    None
                },
            );
        } else {
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

            let view_height = inner.height as usize;
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
            spans.push(Span::styled(
                tab.clone(),
                accent_style().add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(tab.clone(), muted_style()));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn response_status_title(app: &TuiApp) -> Line<'static> {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled("Response", Style::default().fg(Color::White)),
    ];

    if let Some(status) = app.response.status {
        spans.push(Span::styled(" · ", muted_style()));
        let color = match status {
            200..=299 => GREEN,
            300..=399 => BLUE,
            400..=499 => YELLOW,
            500..=599 => RED,
            _ => MUTED,
        };
        spans.push(Span::styled(
            format!("HTTP {status}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::raw(" "));
    Line::from(spans).right_aligned()
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

pub fn apply_selection<'a>(text: Text<'a>, selection: Option<Selection>) -> Text<'a> {
    let Some(selection) = selection else {
        return text;
    };
    let (mut start, mut end) = (selection.start, selection.end);
    if start.0 > end.0 || (start.0 == end.0 && start.1 > end.1) {
        std::mem::swap(&mut start, &mut end);
    }

    let mut new_lines = Vec::new();
    for (line_idx, line) in text.lines.into_iter().enumerate() {
        if line_idx < start.0 || line_idx > end.0 {
            new_lines.push(line);
            continue;
        }

        let mut new_spans = Vec::new();
        let mut current_col = 0;
        for span in line.spans {
            let span_len = span.content.chars().count();
            let span_start = current_col;
            let span_end = current_col + span_len;

            let sel_start_col = if line_idx == start.0 { start.1 } else { 0 };
            let sel_end_col = if line_idx == end.0 { end.1 } else { usize::MAX };

            if span_end <= sel_start_col || span_start > sel_end_col {
                new_spans.push(span);
            } else if span_start >= sel_start_col && span_end <= sel_end_col.saturating_add(1) {
                new_spans.push(Span::styled(
                    span.content,
                    span.style.add_modifier(Modifier::REVERSED),
                ));
            } else {
                let mut current_str = String::new();
                let mut is_reversed = false;
                for (i, c) in span.content.chars().enumerate() {
                    let col = span_start + i;
                    let selected = col >= sel_start_col && col <= sel_end_col;
                    if selected != is_reversed && !current_str.is_empty() {
                        let style = if is_reversed {
                            span.style.add_modifier(Modifier::REVERSED)
                        } else {
                            span.style
                        };
                        new_spans.push(Span::styled(current_str.clone(), style));
                        current_str.clear();
                    }
                    is_reversed = selected;
                    current_str.push(c);
                }
                if !current_str.is_empty() {
                    let style = if is_reversed {
                        span.style.add_modifier(Modifier::REVERSED)
                    } else {
                        span.style
                    };
                    new_spans.push(Span::styled(current_str, style));
                }
            }
            current_col += span_len;
        }
        new_lines.push(Line::from(new_spans));
    }
    Text::from(new_lines)
}

pub fn apply_cursor_to_text(mut text: Text<'static>, cursor: usize, selection: Option<Selection>) -> Text<'static> {
    let mut char_count = 0;
    let mut cursor_applied = false;

    for (line_idx, line) in text.lines.iter_mut().enumerate() {
        let mut new_spans = Vec::new();
        let mut current_col = 0;
        for span in line.spans.drain(..) {
            if cursor_applied {
                new_spans.push(span);
                continue;
            }
            let span_chars = span.content.chars().count();
            if char_count <= cursor && cursor < char_count + span_chars {
                let local_idx = cursor - char_count;
                let col = current_col + local_idx;

                let is_selected = if let Some(sel) = selection {
                    let start = std::cmp::min(sel.start, sel.end);
                    let end = std::cmp::max(sel.start, sel.end);
                    let pos = (line_idx, col);
                    pos >= start && pos <= end
                } else {
                    false
                };

                let byte_idx = span.content.char_indices().nth(local_idx).unwrap().0;
                let (left, right) = span.content.split_at(byte_idx);
                let cursor_char = right.chars().next().unwrap();
                let rest = &right[cursor_char.len_utf8()..];

                if !left.is_empty() {
                    new_spans.push(Span::styled(left.to_string(), span.style));
                }

                let cursor_style = if is_selected {
                    span.style.remove_modifier(Modifier::REVERSED)
                } else {
                    span.style.add_modifier(Modifier::REVERSED)
                };

                new_spans.push(Span::styled(
                    cursor_char.to_string(),
                    cursor_style,
                ));
                if !rest.is_empty() {
                    new_spans.push(Span::styled(rest.to_string(), span.style));
                }
                cursor_applied = true;
            } else {
                new_spans.push(span);
            }
            char_count += span_chars;
            current_col += span_chars;
        }

        if !cursor_applied && char_count == cursor {
            new_spans.push(Span::styled(
                " ",
                Style::default().add_modifier(Modifier::REVERSED),
            ));
            cursor_applied = true;
        }
        char_count += 1; // for newline
        line.spans = new_spans;
    }

    if !cursor_applied {
        if let Some(last) = text.lines.last_mut() {
            last.spans.push(Span::styled(
                " ",
                Style::default().add_modifier(Modifier::REVERSED),
            ));
        } else {
            text.lines.push(Line::from(Span::styled(
                " ",
                Style::default().add_modifier(Modifier::REVERSED),
            )));
        }
    }

    text
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

    #[test]
    fn end_key_moves_body_cursor_to_end_of_current_line() {
        let mut app = TuiApp::new(None);
        app.active_block = ActiveBlock::Params;
        app.active_request_tab = RequestTab::Body;
        app.draft.body = "first\n     asdf   \nthird".to_string();
        app.text_cursor = 8;

        app.handle_key(
            KeyEvent::new(KeyCode::End, crossterm::event::KeyModifiers::NONE),
            None,
        )
        .unwrap();

        assert_eq!(app.text_cursor, "first\n     asdf".chars().count());
    }

    #[test]
    fn home_key_moves_body_cursor_to_start_of_current_line() {
        let mut app = TuiApp::new(None);
        app.active_block = ActiveBlock::Params;
        app.active_request_tab = RequestTab::Body;
        app.draft.body = "first\n     asdf\nthird".to_string();
        app.text_cursor = 14;

        app.handle_key(
            KeyEvent::new(KeyCode::Home, crossterm::event::KeyModifiers::NONE),
            None,
        )
        .unwrap();

        assert_eq!(app.text_cursor, "first\n     ".chars().count());
    }
}

fn insert_char_at(s: &mut String, idx: usize, ch: char) {
    let char_len = s.chars().count();
    let idx = idx.min(char_len);
    if idx == char_len {
        s.push(ch);
    } else {
        let byte_idx = s.char_indices().nth(idx).unwrap().0;
        s.insert(byte_idx, ch);
    }
}

fn remove_char_at(s: &mut String, idx: usize) {
    let char_len = s.chars().count();
    if char_len == 0 || idx == 0 {
        return;
    }
    let idx = idx.min(char_len);
    let byte_idx = s.char_indices().nth(idx - 1).unwrap().0;
    s.remove(byte_idx);
}

fn render_with_cursor_spans(s: &str, cursor: Option<usize>, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if let Some(cursor) = cursor {
        let char_len = s.chars().count();
        let idx = cursor.min(char_len);

        if idx == char_len {
            spans.push(Span::styled(s.to_string(), base_style));
            spans.push(Span::styled(
                " ",
                base_style.add_modifier(Modifier::REVERSED),
            ));
        } else {
            let byte_idx = s.char_indices().nth(idx).unwrap().0;
            let (left, right) = s.split_at(byte_idx);
            let first_char = right.chars().next().unwrap();
            let rest = &right[first_char.len_utf8()..];

            spans.push(Span::styled(left.to_string(), base_style));
            spans.push(Span::styled(
                first_char.to_string(),
                base_style.add_modifier(Modifier::REVERSED),
            ));
            spans.push(Span::styled(rest.to_string(), base_style));
        }
    } else {
        spans.push(Span::styled(s.to_string(), base_style));
    }
    spans
}

fn move_cursor_up(s: &str, cursor: usize) -> usize {
    let char_len = s.chars().count();
    let cursor = cursor.min(char_len);

    let mut current_line_start = 0;
    for (i, c) in s.chars().enumerate() {
        if i == cursor {
            break;
        }
        if c == '\n' {
            current_line_start = i + 1;
        }
    }

    if current_line_start == 0 {
        return 0;
    }

    let col = cursor - current_line_start;

    let mut prev_line_start = 0;
    for (i, c) in s.chars().enumerate() {
        if i == current_line_start - 1 {
            break;
        }
        if c == '\n' {
            prev_line_start = i + 1;
        }
    }

    let prev_line_len = current_line_start - 1 - prev_line_start;
    prev_line_start + col.min(prev_line_len)
}

fn move_cursor_down(s: &str, cursor: usize) -> usize {
    let char_len = s.chars().count();
    let cursor = cursor.min(char_len);

    let mut current_line_start = 0;
    for (i, c) in s.chars().enumerate() {
        if i == cursor {
            break;
        }
        if c == '\n' {
            current_line_start = i + 1;
        }
    }

    let col = cursor - current_line_start;

    let mut next_line_start = char_len;
    for (i, c) in s.chars().enumerate().skip(cursor) {
        if c == '\n' {
            next_line_start = i + 1;
            break;
        }
    }

    if next_line_start == char_len {
        return char_len;
    }

    let mut next_line_len = char_len - next_line_start;
    for (i, c) in s.chars().enumerate().skip(next_line_start) {
        if c == '\n' {
            next_line_len = i - next_line_start;
            break;
        }
    }

    next_line_start + col.min(next_line_len)
}

fn move_cursor_to_line_end(s: &str, cursor: usize) -> usize {
    let (line_start, line_end) = current_line_bounds(s, cursor);
    let mut text_end = line_start;

    for (i, c) in s
        .chars()
        .enumerate()
        .skip(line_start)
        .take(line_end - line_start)
    {
        if !c.is_whitespace() {
            text_end = i + 1;
        }
    }

    if text_end == line_start {
        line_end
    } else {
        text_end
    }
}

fn move_cursor_to_line_start(s: &str, cursor: usize) -> usize {
    let (line_start, line_end) = current_line_bounds(s, cursor);

    for (i, c) in s
        .chars()
        .enumerate()
        .skip(line_start)
        .take(line_end - line_start)
    {
        if !c.is_whitespace() {
            return i;
        }
    }

    line_start
}

fn current_line_bounds(s: &str, cursor: usize) -> (usize, usize) {
    let char_len = s.chars().count();
    let cursor = cursor.min(char_len);
    let mut line_start = 0;
    let mut line_end = char_len;

    for (i, c) in s.chars().enumerate() {
        if c == '\n' {
            if i < cursor {
                line_start = i + 1;
            } else {
                line_end = i;
                break;
            }
        }
    }

    (line_start, line_end)
}

fn render_error_popup(frame: &mut ratatui::Frame, error: &str, popup_area: ratatui::layout::Rect, text_selection: Option<Selection>) {
    use ratatui::widgets::{Block, Borders, Paragraph, Wrap, Clear};
    use ratatui::style::{Style, Color};
    use ratatui::text::Text;

    let block = Block::default()
        .title(" Validation Error ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Red));
        
    let mut text = Text::from(error);
    text = apply_selection(text, text_selection);

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(paragraph, popup_area);
}
