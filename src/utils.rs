use sdl3::pixels::Color;

pub fn parse_color(color_str: &str) -> Option<Color> {
    if color_str.starts_with('#') && color_str.len() == 7 {
        let r: u8 = u8::from_str_radix(&color_str[1..3], 16).ok()?;
        let g: u8 = u8::from_str_radix(&color_str[3..5], 16).ok()?;
        let b: u8 = u8::from_str_radix(&color_str[5..7], 16).ok()?;
        Some(Color::RGB(r, g, b))
    } else {
        None
    }
}
