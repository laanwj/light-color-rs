use crate::app::{App, ControlTarget, Focus, InputMode, MouseAreas};
use crate::color;
use light_protocol::ModeType;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, Paragraph, Tabs},
};

pub fn draw(f: &mut Frame, app: &App) -> MouseAreas {
    let mut mouse_areas = MouseAreas::new();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.area());

    // Header
    let title =
        Paragraph::new("Light Control - 'q' to quit, '↑↓←→' to navigate, 'Tab' mode, 'Enter' edit")
            .style(app.theme.title_style)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
    f.render_widget(title, chunks[0]);

    // Main area layout
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(chunks[1]);

    // Left: Light List
    draw_light_list(f, app, main_chunks[0], &mut mouse_areas);

    // Right: Controls
    draw_controls(f, app, main_chunks[1], &mut mouse_areas);

    mouse_areas
}

fn draw_light_list(f: &mut Frame, app: &App, area: Rect, mouse_areas: &mut MouseAreas) {
    let mut items: Vec<ListItem> = Vec::new();
    for (i, light) in app.lights.iter().enumerate() {
        let is_selected = app.selected_indices.contains(&i);
        let checkbox = if is_selected { "[x] " } else { "[ ] " };

        let color = color::compute_preview(light);

        // Create a span for the checkbox and text
        let checkbox = Span::raw(checkbox);
        let name = Span::raw(format!(" Light #{}", i + 1));

        // Create a span for the color preview
        // We use a block character and set its fg/bg
        let preview = Span::styled("   ", Style::default().bg(color));

        let content = Line::from(vec![checkbox, preview, name]);

        let style = if app.focus == Focus::LightList && app.list_cursor == i {
            app.theme.focus_item
        } else {
            app.theme.normal_item
        };
        items.push(ListItem::new(content).style(style));
        mouse_areas
            .lights
            .push((i, Rect::new(area.x, area.y + 1 + i as u16, area.width, 1)));
    }

    let block = Block::default()
        .title("Lights")
        .borders(Borders::ALL)
        .border_style(if app.focus == Focus::LightList {
            app.theme.focus_control
        } else {
            app.theme.normal_control
        });

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_controls(f: &mut Frame, app: &App, area: Rect, mouse_areas: &mut MouseAreas) {
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
        .highlight_style(app.theme.focus_item)
        .divider("|");

    f.render_widget(tabs, chunks[0]);
    // Mouse areas (TODO is there a more precise way?)
    mouse_areas.modes.push((
        ModeType::CCT,
        Rect::new(chunks[0].x + 2, chunks[0].y + 1, 3, 1),
    ));
    mouse_areas.modes.push((
        ModeType::HSI,
        Rect::new(chunks[0].x + 8, chunks[0].y + 1, 3, 1),
    ));

    // Control Sliders
    let controls_area = chunks[1];

    match app.current_mode {
        ModeType::CCT => draw_cct_controls(f, app, controls_area, mouse_areas),
        ModeType::HSI => draw_hsi_controls(f, app, controls_area, mouse_areas),
    }
}

fn draw_cct_controls(f: &mut Frame, app: &App, area: Rect, mouse_areas: &mut MouseAreas) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(4),
                Constraint::Length(4),
                Constraint::Length(4),
            ]
            .as_ref(),
        )
        .split(area);

    draw_slider(
        f,
        app,
        chunks[0],
        "Dimmer (0-100)",
        app.dim as i16,
        0,
        100,
        ControlTarget::Dim,
        mouse_areas,
    );
    draw_slider(
        f,
        app,
        chunks[1],
        "Color Temp (2700-7500K)",
        app.ct as i16,
        2700,
        7500,
        ControlTarget::CT,
        mouse_areas,
    );
    draw_slider(
        f,
        app,
        chunks[2],
        "Green/Magenta (-100-100)",
        app.gm as i16,
        -100,
        100,
        ControlTarget::GM,
        mouse_areas,
    );
}

fn draw_hsi_controls(f: &mut Frame, app: &App, area: Rect, mouse_areas: &mut MouseAreas) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(4),
                Constraint::Length(4),
                Constraint::Length(4),
            ]
            .as_ref(),
        )
        .split(area);

    draw_slider(
        f,
        app,
        chunks[0],
        "Hue (0-360)",
        app.hue as i16,
        0,
        360,
        ControlTarget::Hue,
        mouse_areas,
    );
    draw_slider(
        f,
        app,
        chunks[1],
        "Saturation (0-100)",
        app.sat as i16,
        0,
        100,
        ControlTarget::Sat,
        mouse_areas,
    );
    draw_slider(
        f,
        app,
        chunks[2],
        "Intensity (0-100)",
        app.dim as i16,
        0,
        100,
        ControlTarget::Int,
        mouse_areas,
    );
}

fn draw_slider(
    f: &mut Frame,
    app: &App,
    area: Rect,
    label: &str,
    value: i16,
    min: i16,
    max: i16,
    target: ControlTarget,
    mouse_areas: &mut MouseAreas,
) {
    let is_focused = if let Focus::Control(t) = app.focus {
        t == target
    } else {
        false
    };

    let border_style = if is_focused {
        if app.input_mode == InputMode::Editing {
            app.theme.edit_control
        } else {
            app.theme.focus_control
        }
    } else {
        app.theme.normal_control
    };

    let block = Block::default()
        .title(format!("{}: {}", label, value))
        .borders(Borders::ALL)
        .border_style(border_style);

    // Normalize value for gauge (0.0 to 1.0)
    let range = max as f64 - min as f64;
    let normalized = if range == 0.0 {
        0.0
    } else {
        ((value - min) as f64 / range).clamp(0.0, 1.0)
    };

    let gauge = Gauge::default()
        .block(block)
        .gauge_style(app.theme.gauge_style)
        .ratio(normalized);

    // Render the gauge in the bottom 3 rows
    let gauge_area = Rect {
        x: area.x,
        y: area.y + 1,
        height: 4,
        width: area.width,
    };
    f.render_widget(gauge, gauge_area);
    mouse_areas.sliders.push((target, gauge_area));

    // Render the gradient ribbon on top
    let ribbon_area = Rect {
        x: area.x + 1, // borders
        y: area.y + 2,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    render_gradient_ribbon(f, ribbon_area, target, min, max);
}

fn compute_ribbon_gradient(target: ControlTarget, value: i16) -> Color {
    match target {
        ControlTarget::Hue => {
            let (r, g, b) = color::hsi_to_rgb(value as u16, 100, 100);
            Color::Rgb(r, g, b)
        }
        ControlTarget::CT => {
            let (r, g, b) = color::kelvin_to_rgb(value as u16);
            Color::Rgb(r, g, b)
        }
        ControlTarget::GM => {
            let (r, g, b) = color::apply_gm((255, 255, 255), value);
            Color::Rgb(r, g, b)
        }
        ControlTarget::Sat => {
            let (r, g, b) = color::hsi_to_rgb(0, value as u16, 50); // Show saturation effect on red?
            Color::Rgb(r, g, b)
        }
        ControlTarget::Int | ControlTarget::Dim => {
            let v = (value as f32 / 100.0 * 255.0) as u8;
            Color::Rgb(v, v, v)
        }
    }
}

fn render_gradient_ribbon(f: &mut Frame, area: Rect, target: ControlTarget, min: i16, max: i16) {
    if area.width < 1 {
        return;
    }

    let mut spans = Vec::new();
    for x in 0..area.width {
        // Map x to value
        let pct = x as f32 / (area.width - 1) as f32;
        let value = (min as f32 + (max as f32 - min as f32) * pct) as i16;
        let color = compute_ribbon_gradient(target, value);

        spans.push(Span::styled(" ", Style::default().bg(color)));
    }

    let line = Line::from(spans);
    f.render_widget(Paragraph::new(line), area);
}
