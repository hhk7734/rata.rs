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
    pub drag_target: DragTarget,
    pub selected_request_row: usize,
    pub editing_param_key: Option<String>,
    pub param_edit_mode: ParamEditMode,
    pub text_cursor: usize,
    pub selected_example_row: usize,
    pub request_scroll: u16,
    pub drag_last_row: Option<u16>,
    pub text_selection: Option<Selection>,
    pub clipboard: Option<arboard::Clipboard>,
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
                headers: Vec::new(),
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
            request_height: 5,
            response_height_percent: 66,
            examples_dropdown_open: false,
            drag_target: DragTarget::None,
            selected_request_row: 0,
            editing_param_key: None,
            param_edit_mode: ParamEditMode::None,
            text_cursor: 0,
            selected_example_row: 0,
            request_scroll: 0,
            drag_last_row: None,
            text_selection: None,
            clipboard: arboard::Clipboard::new().ok(),
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
        self.response_scroll =
            std::cmp::min(self.response_scroll.saturating_add(amount), max_scroll);
    }

    pub fn scroll_request_up(&mut self, amount: u16) {
        self.request_scroll = self.request_scroll.saturating_sub(amount);
    }

    pub fn scroll_request_down(&mut self, amount: u16) {
        if self.active_request_tab == RequestTab::Body {
            let lines = self.draft.body.lines().count() as u16;
            let view_height = self.params_area.height.saturating_sub(2);
            let max_scroll = lines.saturating_sub(view_height);
            self.request_scroll =
                std::cmp::min(self.request_scroll.saturating_add(amount), max_scroll);
        }
    }

    pub fn edit_url(&mut self, url: impl Into<String>) {
        self.draft.url = url.into();
    }

    pub fn send(&mut self, project: Option<&RataProject>) -> anyhow::Result<()> {
        self.response_scroll = 0;
        self.response = ResponseView {
            status: None,
            body: String::new(),
            headers: Vec::new(),
            cookies: Vec::new(),
            error: None,
        };

        match self.send_request(project) {
            Ok(response) => self.response = response,
            Err(error) => {
                self.response.error = Some(error.to_string());
                return Err(error);
            }
        }

        Ok(())
    }

    
    fn get_current_text_len(&self, _project: Option<&RataProject>) -> usize {
        if self.active_block == ActiveBlock::Request {
            return self.draft.url.chars().count();
        } else if self.active_block == ActiveBlock::Params {
            if self.active_request_tab == RequestTab::Body {
                return self.draft.body.chars().count();
            } else if self.param_edit_mode != ParamEditMode::None {
                let map = if self.active_request_tab == RequestTab::Query { &self.draft.params } else { &self.draft.headers };
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
            if self.active_block == ActiveBlock::Params && self.active_request_tab != RequestTab::Body {
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
            let _ = self.send(project);
            return Ok(AppAction::Continue);
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
                Ok(AppAction::Continue)
            }
            KeyCode::Tab => {
                if self.active_block == ActiveBlock::Params && self.active_request_tab != RequestTab::Body && self.param_edit_mode != ParamEditMode::None {
                    let map = if self.active_request_tab == RequestTab::Query { &self.draft.params } else { &self.draft.headers };
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
                Ok(AppAction::Continue)
            }
            KeyCode::Right => {
                let len = self.get_current_text_len(project);
                self.text_cursor = self.text_cursor.min(len).saturating_add(1);
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
                    } else if self.param_edit_mode == ParamEditMode::None {
                        self.selected_request_row = self.selected_request_row.saturating_sub(1);
                        self.text_cursor = usize::MAX;
                    } else {
                        self.text_cursor = 0;
                    }
                } else if self.active_block == ActiveBlock::Examples {
                    self.selected_example_row = self.selected_example_row.saturating_sub(1);
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
                    } else if self.param_edit_mode == ParamEditMode::None {
                        let max = if self.active_request_tab == RequestTab::Query { self.draft.params.len() } else { self.draft.headers.len() };
                        self.selected_request_row = self.selected_request_row.saturating_add(1).min(max);
                        self.text_cursor = usize::MAX;
                    } else {
                        self.text_cursor = usize::MAX;
                    }
                } else if self.active_block == ActiveBlock::Examples {
                    let max = self.model.examples.len().saturating_sub(1);
                    self.selected_example_row = self.selected_example_row.saturating_add(1).min(max);
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Enter => {
                if self.active_block == ActiveBlock::Request {
                    let _ = self.send(project);
                } else if self.active_block == ActiveBlock::Params && self.active_request_tab == RequestTab::Body {
                    insert_char_at(&mut self.draft.body, self.text_cursor, '\n');
                    self.text_cursor = self.text_cursor.saturating_add(1);
                } else if self.active_block == ActiveBlock::Params && self.active_request_tab != RequestTab::Body && self.param_edit_mode == ParamEditMode::None {
                    let map = if self.active_request_tab == RequestTab::Query { &mut self.draft.params } else { &mut self.draft.headers };
                    if self.selected_request_row == map.len() {
                        map.push(ParamState { key: String::new(), value: String::new(), enabled: true, required: false });
                        self.param_edit_mode = ParamEditMode::Key;
                        self.text_cursor = usize::MAX;
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
                }
                Ok(AppAction::Continue)
            }
            KeyCode::Backspace => {
                if self.active_block == ActiveBlock::Request {
                    remove_char_at(&mut self.draft.url, self.text_cursor);
                    self.text_cursor = self.text_cursor.saturating_sub(1);
                } else if self.active_block == ActiveBlock::Params {
                    if self.active_request_tab == RequestTab::Body {
                        remove_char_at(&mut self.draft.body, self.text_cursor);
                        self.text_cursor = self.text_cursor.saturating_sub(1);
                    } else if self.param_edit_mode != ParamEditMode::None {
                        let map = if self.active_request_tab == RequestTab::Query { &mut self.draft.params } else { &mut self.draft.headers };
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
                if value == 'k' && (self.active_block == ActiveBlock::Collections || self.active_block == ActiveBlock::Examples || self.active_block == ActiveBlock::Response || (self.active_block == ActiveBlock::Params && self.param_edit_mode == ParamEditMode::None && self.active_request_tab != RequestTab::Body)) {
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
                if value == 'j' && (self.active_block == ActiveBlock::Collections || self.active_block == ActiveBlock::Examples || self.active_block == ActiveBlock::Response || (self.active_block == ActiveBlock::Params && self.param_edit_mode == ParamEditMode::None && self.active_request_tab != RequestTab::Body)) {
                    if self.active_block == ActiveBlock::Response {
                        self.scroll_response_down(1);
                    } else if self.active_block == ActiveBlock::Collections {
                        self.select_next_operation(project);
                    } else if self.active_block == ActiveBlock::Params {
                        self.selected_request_row = self.selected_request_row.saturating_add(1);
                    } else if self.active_block == ActiveBlock::Examples {
                        let max = self.model.examples.len().saturating_sub(1);
                        self.selected_example_row = self.selected_example_row.saturating_add(1).min(max);
                    }
                    return Ok(AppAction::Continue);
                }
                if value == ' ' && self.active_block == ActiveBlock::Params && self.active_request_tab != RequestTab::Body && self.param_edit_mode == ParamEditMode::None {
                    let map = if self.active_request_tab == RequestTab::Query { &mut self.draft.params } else { &mut self.draft.headers };
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
                } else if self.active_block == ActiveBlock::Params {
                    if self.active_request_tab == RequestTab::Body {
                        insert_char_at(&mut self.draft.body, self.text_cursor, value);
                        self.text_cursor = self.text_cursor.saturating_add(1);
                    } else if self.param_edit_mode != ParamEditMode::None {
                        let map = if self.active_request_tab == RequestTab::Query { &mut self.draft.params } else { &mut self.draft.headers };
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
                        self.draft.params.push(ParamState { key: k, value: v, enabled: true, required: false });
                    }
                }
            }
            if let Some(headers) = data.headers {
                for (k, v) in headers {
                    if let Some(p) = self.draft.headers.iter_mut().find(|p| p.key == k) {
                        p.value = v;
                        p.enabled = true;
                    } else {
                        self.draft.headers.push(ParamState { key: k, value: v, enabled: true, required: false });
                    }
                }
            }
            if let Some(body) = data.body {
                self.draft.body = serde_json::to_string_pretty(&body).unwrap_or_default();
            } else {
                self.draft.body.clear();
            }
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
            let p = ParamState { key: param.name.clone(), value: String::new(), enabled: param.required, required: param.required };
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

                        if let Some(sel) = &mut self.text_selection {
                            let line = if mouse.row < inner_y {
                                self.response_scroll as usize
                            } else {
                                mouse.row.saturating_sub(inner_y) as usize
                                    + self.response_scroll as usize
                            };
                            let col = mouse.column.saturating_sub(inner_x) as usize;
                            sel.end = (line, col);
                        }
                    }
                    DragTarget::None => {}
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
                }
                self.drag_target = DragTarget::None;
                return;
            }
            MouseEventKind::Down(MouseButton::Left) => {
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
                    if self.active_response_tab == ResponseTab::Body {
                        self.drag_target = DragTarget::ResponseSelection;
                        let inner_y = self.response_area.y + 1;
                        let inner_x = self.response_area.x + 1;
                        let line = mouse.row.saturating_sub(inner_y) as usize
                            + self.response_scroll as usize;
                        let col = mouse.column.saturating_sub(inner_x) as usize;
                        self.text_selection = Some(Selection {
                            start: (line, col),
                            end: (line, col),
                        });
                    } else {
                        self.drag_target = DragTarget::ScrollResponse;
                    }
                    self.drag_last_row = Some(mouse.row);
                } else if contains(self.params_area, mouse.column, mouse.row) {
                    self.drag_target = DragTarget::ScrollRequest;
                    self.drag_last_row = Some(mouse.row);
                }
            }
            _ => return,
        }

        self.active_block = ActiveBlock::None;

        let dropdown_x = self.request_area.right().saturating_sub(14);
        let clicked_dropdown_toggle = mouse.row == self.request_area.y
            && mouse.column >= dropdown_x
            && mouse.column < self.request_area.right();
        let clicked_inside_dropdown =
            self.examples_dropdown_open && contains(self.examples_area, mouse.column, mouse.row);

        if self.examples_dropdown_open && !clicked_dropdown_toggle && !clicked_inside_dropdown {
            self.examples_dropdown_open = false;
        }

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
                self.text_cursor = usize::MAX;
            if clicked_dropdown_toggle {
                self.examples_dropdown_open = !self.examples_dropdown_open;
                if self.examples_dropdown_open {
                    self.active_block = ActiveBlock::Examples;
                }
            }
        } else if contains(self.params_area, mouse.column, mouse.row) {
            self.active_block = ActiveBlock::Params;
                self.text_cursor = usize::MAX;
        } else if clicked_inside_dropdown {
            self.active_block = ActiveBlock::Examples;
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
        } else if contains(self.response_area, mouse.column, mouse.row) {
            self.active_block = ActiveBlock::Response;
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
            format!(" Query ({}) ", self.draft.params.iter().filter(|p| p.enabled).count()),
            " Body ".to_string(),
            format!(" Headers ({}) ", self.draft.headers.iter().filter(|p| p.enabled).count()),
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

    pub fn extract_selection(&self, sel: Selection) -> String {
        let text = pretty_body(&self.response.body);
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

    pub fn active_response_text(&self) -> Text<'static> {
        if let Some(error) = &self.response.error {
            return Text::raw(error.clone());
        }

        match self.active_response_tab {
            ResponseTab::Body => {
                let body = pretty_body(&self.response.body);
                apply_selection(highlight_json(&body), self.text_selection)
            }
            ResponseTab::Headers => Text::raw(format_pairs(&self.response.headers, "No headers")),
            ResponseTab::Cookies => {
                if self.response.cookies.is_empty() {
                    Text::raw("No cookies".to_string())
                } else {
                    Text::raw(self.response.cookies.join("\n"))
                }
            }
        }
    }

    fn send_request(&self, project: Option<&RataProject>) -> anyhow::Result<ResponseView> {
        let client = reqwest::blocking::Client::new();

        let mut final_url = self.draft.url.clone();
        if let Some(project) = project {
            let mut variables = project.variables();
            if !variables.contains_key("baseUrl") {
                let server = project.server_url().unwrap_or_default().trim_end_matches('/');
                variables.insert("baseUrl".to_string(), server.to_string());
            }
            for (k, v) in variables {
                let p1 = format!("{{{{{}}}}}", k);
                final_url = final_url.replace(&p1, &v);
            }
        }
        let mut query_params = Vec::new();

        for param in &self.draft.params {
            if !param.enabled || param.key.is_empty() { continue; }
            let p1 = format!("{{{{{}}}}}", param.key);
            let p2 = format!("{{{}}}", param.key);
            if final_url.contains(&p1) || final_url.contains(&p2) {
                final_url = final_url.replace(&p1, &param.value);
                final_url = final_url.replace(&p2, &param.value);
            } else if !param.value.is_empty() {
                query_params.push((&param.key, &param.value));
            }
        }

        let mut request = client.request(self.draft.method.reqwest(), &final_url);

        if !query_params.is_empty() {
            request = request.query(&query_params);
        }

        for param in &self.draft.headers {
            if !param.enabled || param.key.is_empty() { continue; }
            let mut final_value = param.value.clone();
            if let Some(project) = project {
                for (k, v) in project.variables() {
                    let p1 = format!("{{{{{}}}}}", k);
                    final_value = final_value.replace(&p1, &v);
                }
            }
            request = request.header(&param.key, &final_value);
        }

        let mut final_body = self.draft.body.clone();
        if let Some(project) = project {
            for (k, v) in project.variables() {
                let p1 = format!("{{{{{}}}}}", k);
                final_body = final_body.replace(&p1, &v);
            }
        }

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
        .map(|operation| {
            format!("{{{{baseUrl}}}}{}", operation.path)
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
    let mut last_mouse_pos: Option<(u16, u16)> = None;
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let app_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(area);

            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(app.collections_width),
                    Constraint::Min(0),
                ])
                .split(app_layout[0]);
            app.collections_area = body[0];
            let main = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(app.request_height), Constraint::Min(0)])
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
                .constraints([Constraint::Min(0)])
                .split(main_rest[0]);

            app.request_area = main[0];
            app.params_area = request_body[0];
            app.response_area = main_rest[1];

            frame.render_widget(collections(project, app), body[0]);
            frame.render_widget(request_line(app), main[0]);
            render_request_block(frame, app, project, request_body[0]);
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
                frame.render_widget(examples(project, app), area);
            } else {
                app.examples_area = Rect::default();
            }
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => match app.handle_key(key, project)? {
                    AppAction::Quit => return Ok(()),
                    AppAction::Continue => {}
                },
                Event::Mouse(mouse) => {
                    last_mouse_pos = Some((mouse.column, mouse.row));
                    app.handle_mouse(mouse, project);
                }
                _ => {}
            }
        } else {
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
                    app.handle_mouse(synthetic_mouse, project);
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
                .title(Span::styled(" Collections ", Style::default().fg(Color::White)))
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

    let mut spans = vec![
        Span::styled(
            format!(" {} ▾ ", app.draft.method.label()),
            method_style(app.draft.method).bg(SELECTED_BG).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(" ", Style::default().fg(TEXT).bg(SELECTED_BG)),
    ];
    if app.active_block == ActiveBlock::Request {
        spans.extend(render_with_cursor_spans(&url, app.text_cursor, Style::default().fg(TEXT).bg(SELECTED_BG)));
    } else {
        spans.push(Span::styled(url.clone(), Style::default().fg(TEXT).bg(SELECTED_BG)));
    }
    spans.push(Span::styled(" ", Style::default().fg(TEXT).bg(SELECTED_BG)));

    Paragraph::new(Line::from(spans))
    .block(
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
        .title_top(Line::from(Span::styled(" Request ", Style::default().fg(Color::White))).right_aligned())
        .borders(Borders::ALL)
        .style(Style::default().bg(PANEL).fg(TEXT))
        .border_style(border_style);

    match app.active_request_tab {
        RequestTab::Body => {
            let text =
                if app.draft.body.is_empty() && !(app.active_block == ActiveBlock::Params && app.active_request_tab == RequestTab::Body) {
                    "No request body".to_string()
                } else {
                    app.draft.body.clone()
                };
            let mut highlighted = highlight_json(&text);
            if app.active_block == ActiveBlock::Params && app.active_request_tab == RequestTab::Body {
                highlighted = apply_cursor_to_text(highlighted, app.text_cursor);
            }
            let p = Paragraph::new(highlighted)
                .style(Style::default().fg(TEXT))
                .block(block)
                .scroll((app.request_scroll, 0));
            frame.render_widget(p, area);
        }
        RequestTab::Query => {
            let mut rows = Vec::new();
            let params = &app.draft.params;
            for (i, param) in params.iter().enumerate() {
                let display_key = if app.active_block == ActiveBlock::Params && app.param_edit_mode == ParamEditMode::Key && i == app.selected_request_row {
                    Line::from(render_with_cursor_spans(&param.key, app.text_cursor, Style::default()))
                } else {
                    Line::from(param.key.clone())
                };
                let display_value = if app.active_block == ActiveBlock::Params && app.param_edit_mode == ParamEditMode::Value && i == app.selected_request_row {
                    Line::from(render_with_cursor_spans(&param.value, app.text_cursor, Style::default()))
                } else {
                    Line::from(param.value.clone())
                };
                let checkbox_text = if param.enabled { "[x]" } else { "[ ]" }.to_string();
                let checkbox_cell = if param.required { ratatui::widgets::Cell::from(checkbox_text).style(Style::default().fg(MUTED)) } else { ratatui::widgets::Cell::from(checkbox_text) };
                let mut row = Row::new(vec![ checkbox_cell, ratatui::widgets::Cell::from(display_key), ratatui::widgets::Cell::from(display_value), ratatui::widgets::Cell::from("") ]);
                if app.active_block == ActiveBlock::Params && i == app.selected_request_row { row = row.style(Style::default().bg(SELECTED_BG)); }
                rows.push(row);
            }
            let add_style = if app.active_block == ActiveBlock::Params && app.selected_request_row == params.len() { Style::default().bg(SELECTED_BG) } else { Style::default() };
            rows.push(Row::new(vec![ ratatui::widgets::Cell::from(""), ratatui::widgets::Cell::from("<Add new query...>").style(add_style.fg(MUTED)), ratatui::widgets::Cell::from(""), ratatui::widgets::Cell::from("") ]).style(add_style));

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
                let display_key = if app.active_block == ActiveBlock::Params && app.param_edit_mode == ParamEditMode::Key && i == app.selected_request_row {
                    Line::from(render_with_cursor_spans(&param.key, app.text_cursor, Style::default()))
                } else {
                    Line::from(param.key.clone())
                };
                let display_value = if app.active_block == ActiveBlock::Params && app.param_edit_mode == ParamEditMode::Value && i == app.selected_request_row {
                    Line::from(render_with_cursor_spans(&param.value, app.text_cursor, Style::default()))
                } else {
                    Line::from(param.value.clone())
                };
                let checkbox_text = if param.enabled { "[x]" } else { "[ ]" }.to_string();
                let checkbox_cell = if param.required { ratatui::widgets::Cell::from(checkbox_text).style(Style::default().fg(MUTED)) } else { ratatui::widgets::Cell::from(checkbox_text) };
                let mut row = Row::new(vec![ checkbox_cell, ratatui::widgets::Cell::from(display_key), ratatui::widgets::Cell::from(display_value) ]);
                if app.active_block == ActiveBlock::Params && i == app.selected_request_row { row = row.style(Style::default().bg(SELECTED_BG)); }
                rows.push(row);
            }
            let add_style = if app.active_block == ActiveBlock::Params && app.selected_request_row == params.len() { Style::default().bg(SELECTED_BG) } else { Style::default() };
            rows.push(Row::new(vec![ ratatui::widgets::Cell::from(""), ratatui::widgets::Cell::from("<Add new header...>").style(add_style.fg(MUTED)), ratatui::widgets::Cell::from("") ]).style(add_style));

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
        let text = app.active_response_text();
        let lines = text.lines.len();

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
                area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
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

fn apply_cursor_to_text(mut text: Text<'static>, cursor: usize) -> Text<'static> {
    let mut char_count = 0;
    let mut cursor_applied = false;

    for line in &mut text.lines {
        let mut new_spans = Vec::new();
        for span in line.spans.drain(..) {
            if cursor_applied {
                new_spans.push(span);
                continue;
            }
            let span_chars = span.content.chars().count();
            if char_count <= cursor && cursor < char_count + span_chars {
                let local_idx = cursor - char_count;
                let byte_idx = span.content.char_indices().nth(local_idx).unwrap().0;
                let (left, right) = span.content.split_at(byte_idx);
                let cursor_char = right.chars().next().unwrap();
                let rest = &right[cursor_char.len_utf8()..];

                if !left.is_empty() {
                    new_spans.push(Span::styled(left.to_string(), span.style));
                }
                new_spans.push(Span::styled(cursor_char.to_string(), span.style.add_modifier(Modifier::REVERSED)));
                if !rest.is_empty() {
                    new_spans.push(Span::styled(rest.to_string(), span.style));
                }
                cursor_applied = true;
            } else {
                new_spans.push(span);
            }
            char_count += span_chars;
        }
        
        if !cursor_applied && char_count == cursor {
            new_spans.push(Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)));
            cursor_applied = true;
        }
        char_count += 1; // for newline
        line.spans = new_spans;
    }
    
    if !cursor_applied {
        if let Some(last) = text.lines.last_mut() {
            last.spans.push(Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)));
        } else {
            text.lines.push(Line::from(Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED))));
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

pub fn highlight_json(json: &str) -> Text<'static> {
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
        _ if text.chars().all(|c| {
            c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+'
        }) =>
        {
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
    if char_len == 0 || idx == 0 { return; }
    let idx = idx.min(char_len);
    let byte_idx = s.char_indices().nth(idx - 1).unwrap().0;
    s.remove(byte_idx);
}


fn render_with_cursor_spans(s: &str, cursor: usize, base_style: Style) -> Vec<Span<'static>> {
    let char_len = s.chars().count();
    let idx = cursor.min(char_len);
    let mut spans = Vec::new();

    if idx == char_len {
        spans.push(Span::styled(s.to_string(), base_style));
        spans.push(Span::styled(" ", base_style.add_modifier(Modifier::REVERSED)));
    } else {
        let byte_idx = s.char_indices().nth(idx).unwrap().0;
        let (left, right) = s.split_at(byte_idx);
        let first_char = right.chars().next().unwrap();
        let rest = &right[first_char.len_utf8()..];
        
        spans.push(Span::styled(left.to_string(), base_style));
        spans.push(Span::styled(first_char.to_string(), base_style.add_modifier(Modifier::REVERSED)));
        spans.push(Span::styled(rest.to_string(), base_style));
    }
    spans
}

fn move_cursor_up(s: &str, cursor: usize) -> usize {
    let char_len = s.chars().count();
    let cursor = cursor.min(char_len);
    
    let mut current_line_start = 0;
    for (i, c) in s.chars().enumerate() {
        if i == cursor { break; }
        if c == '\n' { current_line_start = i + 1; }
    }
    
    if current_line_start == 0 { return 0; }
    
    let col = cursor - current_line_start;
    
    let mut prev_line_start = 0;
    for (i, c) in s.chars().enumerate() {
        if i == current_line_start - 1 { break; }
        if c == '\n' { prev_line_start = i + 1; }
    }
    
    let prev_line_len = current_line_start - 1 - prev_line_start;
    prev_line_start + col.min(prev_line_len)
}

fn move_cursor_down(s: &str, cursor: usize) -> usize {
    let char_len = s.chars().count();
    let cursor = cursor.min(char_len);
    
    let mut current_line_start = 0;
    for (i, c) in s.chars().enumerate() {
        if i == cursor { break; }
        if c == '\n' { current_line_start = i + 1; }
    }
    
    let col = cursor - current_line_start;
    
    let mut next_line_start = char_len;
    for (i, c) in s.chars().enumerate().skip(cursor) {
        if c == '\n' {
            next_line_start = i + 1;
            break;
        }
    }
    
    if next_line_start == char_len { return char_len; }
    
    let mut next_line_len = char_len - next_line_start;
    for (i, c) in s.chars().enumerate().skip(next_line_start) {
        if c == '\n' {
            next_line_len = i - next_line_start;
            break;
        }
    }
    
    next_line_start + col.min(next_line_len)
}
