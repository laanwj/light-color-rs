use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use light_protocol::{ModeType, State};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use std::collections::HashSet;

pub struct Theme {
    pub normal_control: Style,
    pub focus_control: Style,
    pub edit_control: Style,
    pub normal_item: Style,
    pub focus_item: Style,
    pub title_style: Style,
    pub gauge_style: Style,
}

pub struct App {
    pub first_connect: bool,
    pub lights: Vec<State>,
    pub selected_indices: HashSet<usize>,
    pub current_mode: ModeType,
    pub input_mode: InputMode,

    // UI State
    pub theme: Theme,
    pub focus: Focus,

    // CCT Controls
    pub dim: u8,
    pub ct: u16,
    pub gm: i8,

    // HSI Controls
    pub hue: u16,
    pub sat: u8,

    pub list_cursor: usize,
}

pub struct MouseAreas {
    pub lights: Vec<(usize, Rect)>,
    pub modes: Vec<(ModeType, Rect)>,
    pub sliders: Vec<(ControlTarget, Rect)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Navigation,
    Editing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    LightList,
    Control(ControlTarget),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlTarget {
    Dim,
    CT,
    GM,
    Hue,
    Sat,
    Int,
}

impl MouseAreas {
    pub fn new() -> MouseAreas {
        MouseAreas {
            lights: Vec::new(),
            modes: Vec::new(),
            sliders: Vec::new(),
        }
    }
}

impl ControlTarget {
    pub fn range(&self) -> (i32, i32) {
        match self {
            ControlTarget::Dim => (0, 100),
            ControlTarget::CT => (2700, 7500),
            ControlTarget::GM => (-100, 100),
            ControlTarget::Hue => (0, 360),
            ControlTarget::Sat => (0, 100),
            ControlTarget::Int => (0, 100),
        }
    }
}

impl App {
    pub fn new() -> App {
        App {
            first_connect: true,
            lights: vec![],
            selected_indices: HashSet::new(),
            list_cursor: 0,
            current_mode: ModeType::CCT,
            input_mode: InputMode::Navigation,
            focus: Focus::LightList,
            theme: Theme {
                normal_control: Style::default(),
                focus_control: Style::default().fg(Color::Cyan),
                edit_control: Style::default().fg(Color::LightCyan),
                normal_item: Style::default(),
                focus_item: Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                title_style: Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                gauge_style: Style::default().fg(Color::Cyan),
            },

            dim: 0,
            ct: 2700,
            gm: 0,

            hue: 0,
            sat: 0,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if self.input_mode == InputMode::Navigation {
            match key.code {
                // 'q' and CTRL-'c' are handled in src/main.rs
                KeyCode::Down | KeyCode::Char('j') => self.move_focus(1),
                KeyCode::Up | KeyCode::Char('k') => self.move_focus(-1),
                KeyCode::Right | KeyCode::Char('l') => self.switch_focus(true),
                KeyCode::Left | KeyCode::Char('h') => self.switch_focus(false),
                KeyCode::Char(' ') => self.toggle_selection(),
                KeyCode::Enter => self.toggle_edit_mode(),
                KeyCode::Tab => self.toggle_mode(),
                _ => {}
            }
        } else {
            // Editing mode
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.input_mode = InputMode::Navigation,
                KeyCode::Right | KeyCode::Char('l') => self.adjust_value(1),
                KeyCode::Left | KeyCode::Char('h') => self.adjust_value(-1),
                KeyCode::Up | KeyCode::Char('k') => self.adjust_value(10),
                KeyCode::Down | KeyCode::Char('j') => self.adjust_value(-10),
                _ => {}
            }
        }
    }

    pub fn handle_mouse_event(&mut self, mouse_areas: &MouseAreas, mouse: MouseEvent) {
        let pos = Position::new(mouse.column, mouse.row);
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            for (index, area) in &mouse_areas.lights {
                if area.contains(pos) {
                    self.list_cursor = *index;
                    self.focus = Focus::LightList;
                    self.toggle_selection();
                }
            }
            for (mode, area) in &mouse_areas.modes {
                if area.contains(pos) {
                    self.current_mode = *mode;
                }
            }
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
            || mouse.kind == MouseEventKind::Drag(MouseButton::Left)
        {
            for (target, area) in &mouse_areas.sliders {
                if area.contains(pos) {
                    self.focus = Focus::Control(*target);
                    let (min, max) = target.range();
                    let val = min
                        + (pos.x as i32 - area.x as i32) * (max - min) / (area.width as i32 - 1);
                    match target {
                        ControlTarget::Dim => self.dim = val as u8,
                        ControlTarget::CT => {
                            self.ct = val as u16;
                        }
                        ControlTarget::GM => {
                            self.gm = val as i8;
                        }
                        ControlTarget::Hue => {
                            self.hue = val as u16;
                        }
                        ControlTarget::Sat => {
                            self.sat = val as u8;
                        }
                        ControlTarget::Int => {
                            self.dim = val as u8;
                        }
                    }
                }
            }
        }
        self.update_selected_lights();
    }

    fn move_focus(&mut self, delta: i32) {
        match self.focus {
            Focus::LightList => {
                if self.lights.is_empty() {
                    return;
                }
                let new_cursor = self.list_cursor as i32 + delta;
                self.list_cursor = new_cursor.clamp(0, self.lights.len() as i32 - 1) as usize;
                self.sync_controls_with_cursor();
            }
            Focus::Control(target) => {
                // Move up/down between controls
                let order = match self.current_mode {
                    ModeType::CCT => vec![ControlTarget::Dim, ControlTarget::CT, ControlTarget::GM],
                    ModeType::HSI => {
                        vec![ControlTarget::Hue, ControlTarget::Sat, ControlTarget::Int]
                    }
                };
                if let Some(pos) = order.iter().position(|&x| x == target) {
                    let new_pos = (pos as i32 + delta).clamp(0, order.len() as i32 - 1) as usize;
                    self.focus = Focus::Control(order[new_pos]);
                }
            }
        }
    }

    fn switch_focus(&mut self, right: bool) {
        if right {
            if self.focus == Focus::LightList {
                // Jump directly to controls
                self.focus = Focus::Control(match self.current_mode {
                    ModeType::CCT => ControlTarget::Dim,
                    ModeType::HSI => ControlTarget::Hue,
                });
            }
        } else {
            match self.focus {
                Focus::Control(_) => self.focus = Focus::LightList,
                _ => {}
            }
        }
    }

    fn toggle_selection(&mut self) {
        if self.focus == Focus::LightList && !self.lights.is_empty() {
            if self.selected_indices.contains(&self.list_cursor) {
                self.selected_indices.remove(&self.list_cursor);
            } else {
                self.selected_indices.insert(self.list_cursor);
            }
        }
    }

    fn toggle_mode(&mut self) {
        self.current_mode = match self.current_mode {
            ModeType::CCT => ModeType::HSI,
            ModeType::HSI => ModeType::CCT,
        };
        // Reset focus to top of controls if we are currently focusing a control
        if let Focus::Control(_) = self.focus {
            self.focus = Focus::Control(match self.current_mode {
                ModeType::CCT => ControlTarget::Dim,
                ModeType::HSI => ControlTarget::Hue,
            });
        }
        self.update_selected_lights();
    }

    fn toggle_edit_mode(&mut self) {
        if let Focus::Control(_) = self.focus {
            self.input_mode = if self.input_mode == InputMode::Navigation {
                InputMode::Editing
            } else {
                InputMode::Navigation
            };
        }
    }

    pub fn sync_controls_with_cursor(&mut self) {
        if self.list_cursor < self.lights.len() {
            let light = &self.lights[self.list_cursor];
            // Sync values
            // We only sync if we are navigating the list, to show "what's there".
            // If we have a multiple selection, showing one might be misleading, but standard TUI/GUI behavior
            // often shows the "lead" selection's value.

            if let Some(mode) = light.mode {
                self.current_mode = mode;
            }
            if let Some(dim) = light.dim {
                self.dim = dim as u8;
            }
            if let Some(ct) = light.ct {
                self.ct = ct;
            }
            if let Some(gm) = light.gm {
                self.gm = gm as i8;
            }
            if let Some(hue) = light.hue {
                self.hue = hue;
            }
            if let Some(sat) = light.sat {
                self.sat = sat as u8;
            }
            // Dim/Int unified
        }
    }

    fn adjust_value(&mut self, delta: i32) {
        match self.focus {
            Focus::Control(control) => {
                let (min, max) = control.range();
                match control {
                    ControlTarget::Dim => {
                        self.dim = (self.dim as i32 + delta).clamp(min, max) as u8
                    }
                    ControlTarget::CT => {
                        let step = if delta.abs() >= 10 {
                            delta * 10
                        } else {
                            delta * 50
                        };
                        self.ct = (self.ct as i32 + step).clamp(min, max) as u16;
                    }
                    ControlTarget::GM => self.gm = (self.gm as i32 + delta).clamp(min, max) as i8,
                    ControlTarget::Hue => {
                        self.hue = (self.hue as i32 + delta).clamp(min, max) as u16
                    }
                    ControlTarget::Sat => {
                        self.sat = (self.sat as i32 + delta).clamp(min, max) as u8
                    }
                    ControlTarget::Int => {
                        self.dim = (self.dim as i32 + delta).clamp(min, max) as u8
                    }
                }
            }
            _ => {}
        }
        self.update_selected_lights();
    }

    fn update_selected_lights(&mut self) {
        for idx in &self.selected_indices {
            if *idx < self.lights.len() {
                let light = &mut self.lights[*idx];
                light.mode = Some(self.current_mode);
                match self.current_mode {
                    ModeType::CCT => {
                        light.dim = Some(self.dim as u16);
                        light.ct = Some(self.ct);
                        light.gm = Some(self.gm as i16);
                    }
                    ModeType::HSI => {
                        light.hue = Some(self.hue);
                        light.sat = Some(self.sat as u16);
                        light.dim = Some(self.dim as u16);
                    }
                }
            }
        }
    }
}
