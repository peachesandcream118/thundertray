use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

const ICON_SIZE: u32 = 24;
const GLYPH_WIDTH: i32 = 3;
const GLYPH_HEIGHT: i32 = 5;
const GLYPH_GAP: i32 = 1;

pub struct IconPixmap {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>, // ARGB32 big-endian per SNI spec
}

// Compact 3×5 bitmap glyphs for 1–99 and 99+. The original 5×7 glyphs were
// wider than the 14px badge, so the third character in "99+" was clipped.
const DIGIT_BITMAPS: [[u8; GLYPH_HEIGHT as usize]; 11] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
    [0b000, 0b010, 0b111, 0b010, 0b000], // +
];

fn parse_hex_color(hex: &str, fallback: Color) -> Color {
    let hex = hex.trim().trim_start_matches('#');
    let rgb = match hex.len() {
        3 => {
            let mut digits = hex.bytes().map(hex_value);
            match (
                digits.next().flatten(),
                digits.next().flatten(),
                digits.next().flatten(),
            ) {
                (Some(r), Some(g), Some(b)) => Some((r * 17, g * 17, b * 17)),
                _ => None,
            }
        }
        6 => {
            let bytes = hex.as_bytes();
            match (
                hex_value(bytes[0]).zip(hex_value(bytes[1])),
                hex_value(bytes[2]).zip(hex_value(bytes[3])),
                hex_value(bytes[4]).zip(hex_value(bytes[5])),
            ) {
                (Some((rh, rl)), Some((gh, gl)), Some((bh, bl))) => {
                    Some((rh * 16 + rl, gh * 16 + gl, bh * 16 + bl))
                }
                _ => None,
            }
        }
        _ => None,
    };

    rgb.map_or(fallback, |(r, g, b)| Color::from_rgba8(r, g, b, 255))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn render_icon(count: u32, badge_color: &str, text_color: &str) -> IconPixmap {
    let mut pixmap = Pixmap::new(ICON_SIZE, ICON_SIZE).expect("24px icon allocation must succeed");

    // Draw envelope body: filled rectangle, color #4A90D9 (blue).
    let envelope_color = Color::from_rgba8(0x4A, 0x90, 0xD9, 255);
    let envelope_rect = Rect::from_xywh(2.0, 6.0, 20.0, 14.0).expect("valid envelope rectangle");
    let mut paint = Paint::default();
    paint.set_color(envelope_color);
    pixmap.fill_rect(envelope_rect, &paint, Transform::identity(), None);

    // Draw envelope flap: triangle, darker blue #3A7BC8.
    let flap_color = Color::from_rgba8(0x3A, 0x7B, 0xC8, 255);
    let mut flap = PathBuilder::new();
    flap.move_to(2.0, 6.0);
    flap.line_to(12.0, 13.0);
    flap.line_to(22.0, 6.0);
    flap.close();
    paint.set_color(flap_color);
    pixmap.fill_path(
        &flap.finish().expect("valid flap path"),
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    if count > 0 {
        let badge_color = parse_hex_color(badge_color, Color::from_rgba8(0xE5, 0x39, 0x35, 255));
        let text_color = parse_hex_color(text_color, Color::from_rgba8(255, 255, 255, 255));
        paint.set_color(badge_color);

        let text = if count > 99 {
            "99+".to_owned()
        } else {
            count.to_string()
        };

        match text.len() {
            1 => draw_circle(&mut pixmap, &paint, 18.0, 18.0, 4.5),
            2 => draw_pill(&mut pixmap, &paint, 12.0, 14.0, 10.0, 8.0),
            _ => draw_pill(&mut pixmap, &paint, 9.0, 14.0, 13.0, 8.0),
        }

        let center_x = match text.len() {
            1 => 18,
            2 => 17,
            _ => 15,
        };
        draw_text(&mut pixmap, &text, center_x, 18, &text_color);
    }

    IconPixmap {
        width: ICON_SIZE as i32,
        height: ICON_SIZE as i32,
        data: convert_to_argb_be(&pixmap),
    }
}

fn draw_circle(pixmap: &mut Pixmap, paint: &Paint, center_x: f32, center_y: f32, radius: f32) {
    let mut path = PathBuilder::new();
    path.push_circle(center_x, center_y, radius);
    pixmap.fill_path(
        &path.finish().expect("valid circle path"),
        paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_pill(pixmap: &mut Pixmap, paint: &Paint, left: f32, top: f32, width: f32, height: f32) {
    let radius = height / 2.0;
    let center_y = top + radius;
    let middle = Rect::from_xywh(left + radius, top, width - 2.0 * radius, height)
        .expect("valid pill rectangle");
    pixmap.fill_rect(middle, paint, Transform::identity(), None);
    draw_circle(pixmap, paint, left + radius, center_y, radius);
    draw_circle(pixmap, paint, left + width - radius, center_y, radius);
}

fn draw_text(pixmap: &mut Pixmap, text: &str, center_x: i32, center_y: i32, color: &Color) {
    let glyphs = text.chars().count() as i32;
    let text_width = glyphs * GLYPH_WIDTH + (glyphs - 1) * GLYPH_GAP;
    let start_x = center_x - text_width / 2;
    let start_y = center_y - GLYPH_HEIGHT / 2;

    for (glyph_index, ch) in text.chars().enumerate() {
        let glyph = if ch == '+' {
            &DIGIT_BITMAPS[10]
        } else if let Some(digit) = ch.to_digit(10) {
            &DIGIT_BITMAPS[digit as usize]
        } else {
            continue;
        };

        let glyph_x = start_x + glyph_index as i32 * (GLYPH_WIDTH + GLYPH_GAP);
        for (row, glyph_row) in glyph.iter().enumerate() {
            for column in 0..GLYPH_WIDTH {
                if glyph_row & (1 << (GLYPH_WIDTH - 1 - column)) != 0 {
                    let x = glyph_x + column;
                    let y = start_y + row as i32;
                    if x >= 0 && y >= 0 && x < pixmap.width() as i32 && y < pixmap.height() as i32 {
                        set_pixel_premul(pixmap, x as u32, y as u32, color);
                    }
                }
            }
        }
    }
}

fn set_pixel_premul(pixmap: &mut Pixmap, x: u32, y: u32, color: &Color) {
    let idx = ((y * pixmap.width() + x) * 4) as usize;
    let data = pixmap.data_mut();

    let r = (color.red() * 255.0) as u8;
    let g = (color.green() * 255.0) as u8;
    let b = (color.blue() * 255.0) as u8;
    let a = (color.alpha() * 255.0) as u8;

    data[idx] = ((r as u16 * a as u16) / 255) as u8;
    data[idx + 1] = ((g as u16 * a as u16) / 255) as u8;
    data[idx + 2] = ((b as u16 * a as u16) / 255) as u8;
    data[idx + 3] = a;
}

fn convert_to_argb_be(pixmap: &Pixmap) -> Vec<u8> {
    let mut result = Vec::with_capacity((pixmap.width() * pixmap.height() * 4) as usize);

    for pixel in pixmap.data().as_chunks::<4>().0 {
        let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
        let (r, g, b) = if a > 0 {
            (
                ((r as u16 * 255) / a as u16) as u8,
                ((g as u16 * 255) / a as u16) as u8,
                ((b as u16 * 255) / a as u16) as u8,
            )
        } else {
            (r, g, b)
        };
        result.extend_from_slice(&[a, r, g, b]);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_badge_dimensions() {
        let icon = render_icon(0, "#FF0000", "#FFFFFF");
        assert_eq!((icon.width, icon.height), (24, 24));
        assert_eq!(icon.data.len(), (24 * 24 * 4) as usize);
    }

    #[test]
    fn test_with_badge_dimensions() {
        let icon = render_icon(5, "#FF0000", "#FFFFFF");
        assert_eq!((icon.width, icon.height), (24, 24));
        assert_eq!(icon.data.len(), (24 * 24 * 4) as usize);
    }

    #[test]
    fn overflow_badge_fits_all_three_glyphs_inside_the_canvas() {
        let icon = render_icon(150, "#FF0000", "#FFFFFF");
        let icon_ref = &icon;
        let white_pixels = (0..24)
            .flat_map(|y| (0..24).map(move |x| (x, y, pixel(icon_ref, x, y))))
            .filter(|(_, _, pixel)| *pixel == [255, 255, 255, 255])
            .collect::<Vec<_>>();

        // The left "9" and the right '+' both render; the old 5×7 font
        // positioned the '+' beyond the right edge of the 14px badge.
        assert!(white_pixels.iter().any(|(x, _, _)| *x <= 12));
        assert!(white_pixels.iter().any(|(x, _, _)| *x >= 18));

        // The badge has a real canvas margin, so it is not relying on clipping.
        assert!((0..24).all(|y| pixel(&icon, 23, y)[0] == 0));
        assert!((0..24).all(|x| pixel(&icon, x, 23)[0] == 0));
    }

    #[test]
    fn invalid_colors_fall_back_without_panicking() {
        let icon = render_icon(12, "red", "#not-a-colour");
        assert_eq!(icon.data.len(), (24 * 24 * 4) as usize);
    }

    fn pixel(icon: &IconPixmap, x: usize, y: usize) -> [u8; 4] {
        let index = (y * icon.width as usize + x) * 4;
        [
            icon.data[index],
            icon.data[index + 1],
            icon.data[index + 2],
            icon.data[index + 3],
        ]
    }
}
