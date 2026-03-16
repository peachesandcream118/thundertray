use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

pub struct IconPixmap {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>, // ARGB32 big-endian per SNI spec
}

// Bitmap font for digits 0-9 and '+' (index 10)
// Each entry is 7 rows of 5-pixel-wide glyph, packed as bits in u8 (MSB = leftmost pixel)
const DIGIT_BITMAPS: [[u8; 7]; 11] = [
    // 0
    [
        0b01110000, //  ###
        0b10001000, // #   #
        0b10011000, // #  ##
        0b10101000, // # # #
        0b11001000, // ##  #
        0b10001000, // #   #
        0b01110000, //  ###
    ],
    // 1
    [
        0b00100000, //   #
        0b01100000, //  ##
        0b00100000, //   #
        0b00100000, //   #
        0b00100000, //   #
        0b00100000, //   #
        0b01110000, //  ###
    ],
    // 2
    [
        0b01110000, //  ###
        0b10001000, // #   #
        0b00001000, //     #
        0b00110000, //   ##
        0b01000000, //  #
        0b10000000, // #
        0b11111000, // #####
    ],
    // 3
    [
        0b01110000, //  ###
        0b10001000, // #   #
        0b00001000, //     #
        0b00110000, //   ##
        0b00001000, //     #
        0b10001000, // #   #
        0b01110000, //  ###
    ],
    // 4
    [
        0b00010000, //    #
        0b00110000, //   ##
        0b01010000, //  # #
        0b10010000, // #  #
        0b11111000, // #####
        0b00010000, //    #
        0b00010000, //    #
    ],
    // 5
    [
        0b11111000, // #####
        0b10000000, // #
        0b11110000, // ####
        0b00001000, //     #
        0b00001000, //     #
        0b10001000, // #   #
        0b01110000, //  ###
    ],
    // 6
    [
        0b01110000, //  ###
        0b10001000, // #   #
        0b10000000, // #
        0b11110000, // ####
        0b10001000, // #   #
        0b10001000, // #   #
        0b01110000, //  ###
    ],
    // 7
    [
        0b11111000, // #####
        0b00001000, //     #
        0b00010000, //    #
        0b00100000, //   #
        0b00100000, //   #
        0b00100000, //   #
        0b00100000, //   #
    ],
    // 8
    [
        0b01110000, //  ###
        0b10001000, // #   #
        0b10001000, // #   #
        0b01110000, //  ###
        0b10001000, // #   #
        0b10001000, // #   #
        0b01110000, //  ###
    ],
    // 9
    [
        0b01110000, //  ###
        0b10001000, // #   #
        0b10001000, // #   #
        0b01111000, //  ####
        0b00001000, //     #
        0b10001000, // #   #
        0b01110000, //  ###
    ],
    // +
    [
        0b00000000, //
        0b00100000, //   #
        0b00100000, //   #
        0b11111000, // #####
        0b00100000, //   #
        0b00100000, //   #
        0b00000000, //
    ],
];

fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Color::from_rgba8(r, g, b, 255)
}

pub fn render_icon(count: u32, badge_color: &str, text_color: &str) -> IconPixmap {
    let mut pixmap = Pixmap::new(24, 24).unwrap();

    // Draw envelope body: filled rectangle, color #4A90D9 (blue)
    let envelope_color = Color::from_rgba8(0x4A, 0x90, 0xD9, 255);
    let envelope_rect = Rect::from_xywh(2.0, 6.0, 20.0, 14.0).unwrap();
    let mut paint = Paint::default();
    paint.set_color(envelope_color);
    pixmap.fill_rect(envelope_rect, &paint, Transform::identity(), None);

    // Draw envelope flap: triangle, darker blue #3A7BC8
    let flap_color = Color::from_rgba8(0x3A, 0x7B, 0xC8, 255);
    let mut pb = PathBuilder::new();
    pb.move_to(2.0, 6.0);
    pb.line_to(12.0, 13.0);
    pb.line_to(22.0, 6.0);
    pb.close();
    let path = pb.finish().unwrap();
    paint.set_color(flap_color);
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    // Draw badge if count > 0
    if count > 0 {
        // Draw badge circle: radius 7, center at (18,18)
        let badge_col = parse_hex_color(badge_color);
        paint.set_color(badge_col);

        let mut pb = PathBuilder::new();
        pb.push_circle(18.0, 18.0, 7.0);
        let path = pb.finish().unwrap();
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

        // Draw badge text using bitmap font
        let text = if count > 99 {
            "99+".to_string()
        } else {
            count.to_string()
        };

        draw_text(&mut pixmap, &text, 18, 18, text_color);
    }

    // Convert to ARGB big-endian
    let data = convert_to_argb_be(&pixmap);

    IconPixmap {
        width: 24,
        height: 24,
        data,
    }
}

fn draw_text(pixmap: &mut Pixmap, text: &str, center_x: i32, center_y: i32, color: &str) {
    let col = parse_hex_color(color);

    // Calculate text width (each char is 5 pixels wide)
    let text_width = text.chars().count() * 5;
    let text_height = 7;

    let start_x = center_x - (text_width as i32 / 2);
    let start_y = center_y - (text_height / 2);

    for (char_idx, ch) in text.chars().enumerate() {
        let glyph = if ch == '+' {
            &DIGIT_BITMAPS[10]
        } else if ch.is_ascii_digit() {
            &DIGIT_BITMAPS[ch.to_digit(10).unwrap() as usize]
        } else {
            continue;
        };

        let char_x = start_x + (char_idx as i32 * 5);

        for (row, glyph_row) in glyph.iter().enumerate().take(7) {
            for col_bit in 0..5 {
                if (glyph_row & (1 << (7 - col_bit))) != 0 {
                    let px = char_x + col_bit;
                    let py = start_y + row as i32;

                    if (0..24).contains(&px) && (0..24).contains(&py) {
                        set_pixel_premul(pixmap, px as u32, py as u32, &col);
                    }
                }
            }
        }
    }
}

fn set_pixel_premul(pixmap: &mut Pixmap, x: u32, y: u32, color: &Color) {
    let idx = ((y * 24 + x) * 4) as usize;
    let data = pixmap.data_mut();

    // Convert color to premultiplied RGBA for tiny-skia
    let r = (color.red() * 255.0) as u8;
    let g = (color.green() * 255.0) as u8;
    let b = (color.blue() * 255.0) as u8;
    let a = (color.alpha() * 255.0) as u8;

    // Premultiply
    let r_pre = ((r as u16 * a as u16) / 255) as u8;
    let g_pre = ((g as u16 * a as u16) / 255) as u8;
    let b_pre = ((b as u16 * a as u16) / 255) as u8;

    data[idx] = r_pre;
    data[idx + 1] = g_pre;
    data[idx + 2] = b_pre;
    data[idx + 3] = a;
}

fn convert_to_argb_be(pixmap: &Pixmap) -> Vec<u8> {
    let mut result = Vec::with_capacity((24 * 24 * 4) as usize);
    let data = pixmap.data();

    for i in (0..data.len()).step_by(4) {
        let r = data[i];
        let g = data[i + 1];
        let b = data[i + 2];
        let a = data[i + 3];

        // Unpremultiply: tiny-skia stores premultiplied RGBA
        let (r_u, g_u, b_u) = if a > 0 {
            (
                ((r as u16 * 255) / a as u16) as u8,
                ((g as u16 * 255) / a as u16) as u8,
                ((b as u16 * 255) / a as u16) as u8,
            )
        } else {
            (r, g, b)
        };

        // Write as ARGB big-endian per SNI spec
        result.push(a);
        result.push(r_u);
        result.push(g_u);
        result.push(b_u);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_badge_dimensions() {
        let icon = render_icon(0, "#FF0000", "#FFFFFF");
        assert_eq!(icon.width, 24);
        assert_eq!(icon.height, 24);
        assert_eq!(icon.data.len(), (24 * 24 * 4) as usize);
    }

    #[test]
    fn test_with_badge_dimensions() {
        let icon = render_icon(5, "#FF0000", "#FFFFFF");
        assert_eq!(icon.width, 24);
        assert_eq!(icon.height, 24);
        assert_eq!(icon.data.len(), (24 * 24 * 4) as usize);
    }

    #[test]
    fn test_overflow_badge() {
        let icon = render_icon(150, "#FF0000", "#FFFFFF");
        assert_eq!(icon.width, 24);
        assert_eq!(icon.data.len(), (24 * 24 * 4) as usize);
    }
}
