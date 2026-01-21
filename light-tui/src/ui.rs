use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Gauge, BorderType},
    Frame,
};
use crate::app::{App, InputMode, Focus, ControlTarget};
use light_protocol::ModeType;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(f.area());

    // Header
    let title_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let title = Paragraph::new("Light Control TUI - 'q' to quit, 'Tab' mode, 'Space' select, 'Enter' edit")
        .style(title_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(title, chunks[0]);
    
    // Main area layout
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(chunks[1]);

    // Left: Light List
    draw_light_list(f, app, main_chunks[0]);

    // Right: Controls
    draw_controls(f, app, main_chunks[1]);
}

use crate::color;

fn draw_light_list(f: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();
    for (i, light) in app.lights.iter().enumerate() {
        let is_selected = app.selected_indices.contains(&i);
        let checkbox = if is_selected { "[x] " } else { "[ ] " };
        
        let color = color::compute_preview(light);
        let dim = light.dim.unwrap_or(0);
        
        // Create a span for the checkbox and text
        let text = Span::raw(format!("{} Light #{} ", checkbox, i + 1));
        
        // Create a span for the color preview
        // We use a block character and set its fg/bg
        let preview = Span::styled("   ", Style::default().bg(color));
        
        let content = Line::from(vec![text, preview]);

        let style = if app.focus == Focus::LightList && app.list_cursor == i {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        items.push(ListItem::new(content).style(style));
    }
    
    let block = Block::default()
        .title("Lights")
        .borders(Borders::ALL)
        .border_style(if app.focus == Focus::LightList { Style::default().fg(Color::Yellow) } else { Style::default() });
    
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_controls(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(area);

    // Tabs
    let titles = vec!["CCT", "HSI"];
    let mode_index = match app.current_mode {
        ModeType::CCT => 0,
        ModeType::HSI => 1,
    };
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Mode"))
        .select(mode_index)
        .highlight_style(Style::default().fg(Color::Green))
        .divider("|");
        
    f.render_widget(tabs, chunks[0]);

    // Control Sliders
    let controls_area = chunks[1];
    
    match app.current_mode {
        ModeType::CCT => draw_cct_controls(f, app, controls_area),
        ModeType::HSI => draw_hsi_controls(f, app, controls_area),
    }
}

fn draw_cct_controls(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(3)].as_ref())
        .split(area);
        
    draw_slider(f, app, chunks[0], "Dimmer (0-100)", app.dim as i16, 0, 100, ControlTarget::Dim);
    draw_slider(f, app, chunks[1], "Color Temp (2700-7500K)", app.ct as i16, 2700, 7500, ControlTarget::CT);
    draw_slider(f, app, chunks[2], "Green/Magenta (-100-100)", app.gm as i16, -100, 100, ControlTarget::GM);
}

fn draw_hsi_controls(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(3)].as_ref())
        .split(area);

    draw_slider(f, app, chunks[0], "Hue (0-360)", app.hue as i16, 0, 360, ControlTarget::Hue);
    draw_slider(f, app, chunks[1], "Saturation (0-100)", app.sat as i16, 0, 100, ControlTarget::Sat);
    draw_slider(f, app, chunks[2], "Intensity (0-100)", app.dim as i16, 0, 100, ControlTarget::Int);
}

fn draw_slider(f: &mut Frame, app: &App, area: Rect, label: &str, value: i16, min: i16, max: i16, target: ControlTarget) {
    let is_focused = if let Focus::Control(t) = app.focus { t == target } else { false };
    
    let border_style = if is_focused {
        if app.input_mode == InputMode::Editing {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Yellow)
        }
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(format!("{}: {}", label, value))
        .borders(Borders::ALL)
        .border_style(border_style);
    
    // Normalize value for gauge (0.0 to 1.0)
    let range = max as f64 - min as f64;
    let normalized = if range == 0.0 { 0.0 } else { ((value - min) as f64 / range).clamp(0.0, 1.0) };
    
    let gauge = Gauge::default()
        .block(block)
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(normalized);
        
    f.render_widget(gauge, area);
}
