use light_protocol::{ModeType, State};
use ratatui::style::Color;

pub fn compute_preview(state: &State) -> Color {
    let mode = state.mode.unwrap_or(ModeType::CCT);
    let (r, g, b) = match mode {
        ModeType::CCT => {
            let kelvin = state.ct.unwrap_or(2700);
            let gm = state.gm.unwrap_or(0);
            let dim = state.dim.unwrap_or(0);

            let (r, g, b) = kelvin_to_rgb(kelvin);
            let (r, g, b) = apply_gm((r, g, b), gm);
            apply_dimming((r, g, b), dim)
        }
        ModeType::HSI => {
            let hue = state.hue.unwrap_or(0);
            let sat = state.sat.unwrap_or(0);
            let dim = state.dim.unwrap_or(0); // Assuming dim is intensity

            let (r, g, b) = hsi_to_rgb(hue, sat, dim);
            (r, g, b)
        }
    };
    Color::Rgb(r, g, b)
}

pub fn apply_dimming(rgb: (u8, u8, u8), dim: u16) -> (u8, u8, u8) {
    let factor = (dim as f32 / 100.0).powf(0.25); // Gamma correction from reference
    (
        (rgb.0 as f32 * factor) as u8,
        (rgb.1 as f32 * factor) as u8,
        (rgb.2 as f32 * factor) as u8,
    )
}

pub fn apply_gm(rgb: (u8, u8, u8), gm: i16) -> (u8, u8, u8) {
    if gm == 0 {
        return rgb;
    }

    let (tr, tg, tb) = if gm < 0 {
        (255, 0, 255) // Magenta
    } else {
        (0, 255, 0) // Green
    };

    let strength = (gm.abs() as f32 / 100.0) * 0.2; // GM_SCALE = 0.2

    // Simple blend
    let r = rgb.0 as f32 * (1.0 - strength) + tr as f32 * strength;
    let g = rgb.1 as f32 * (1.0 - strength) + tg as f32 * strength;
    let b = rgb.2 as f32 * (1.0 - strength) + tb as f32 * strength;

    (r as u8, g as u8, b as u8)
}

pub fn hsi_to_rgb(h: u16, s: u16, i: u16) -> (u8, u8, u8) {
    let h_float = h as f32;
    let s_float = s as f32 / 100.0;

    let c = s_float; // Value is 1.0 in from_hsv call
    let x = c * (1.0 - ((h_float / 60.0) % 2.0 - 1.0).abs());
    let m = 1.0 - c; // If V=1, m = 1-C

    let (r, g, b) = if h_float < 60.0 {
        (c, x, 0.0)
    } else if h_float < 120.0 {
        (x, c, 0.0)
    } else if h_float < 180.0 {
        (0.0, c, x)
    } else if h_float < 240.0 {
        (0.0, x, c)
    } else if h_float < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let r = (r + m) * 255.0;
    let g = (g + m) * 255.0;
    let b = (b + m) * 255.0;

    apply_dimming((r as u8, g as u8, b as u8), i)
}

pub fn kelvin_to_rgb(k: u16) -> (u8, u8, u8) {
    // Tanner Helland's algorithm approximation
    let temp = (k as f32).clamp(1000.0, 40000.0) / 100.0;

    let r = if temp <= 66.0 {
        255.0
    } else {
        329.698727446 * (temp - 60.0).powf(-0.1332047592)
    };

    let g = if temp <= 66.0 {
        99.4708025861 * temp.ln() - 161.1195681661
    } else {
        288.1221695283 * (temp - 60.0).powf(-0.0755148492)
    };

    let b = if temp >= 66.0 {
        255.0
    } else if temp <= 19.0 {
        0.0
    } else {
        138.5177312231 * (temp - 10.0).ln() - 305.0447927307
    };

    (
        r.clamp(0.0, 255.0) as u8,
        g.clamp(0.0, 255.0) as u8,
        b.clamp(0.0, 255.0) as u8,
    )
}
