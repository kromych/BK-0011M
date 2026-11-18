//! Screen rendering from video RAM.
//!
//! Each screen is 256 lines of 64 bytes. Monochrome: 512 pixels per line,
//! 1 bit each. Color: 256 pixels per line, 2 bits each. In both modes the
//! least significant bits are the leftmost pixels.

/// Output frame buffer size. Color pixels are doubled horizontally.
pub const SCREEN_WIDTH: usize = 512;
pub const SCREEN_HEIGHT: usize = 256;

const LINE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Monitor {
    #[default]
    Color,
    Mono,
}

/// 0x00RRGGBB colors for the 16 hardware palettes; color 0 is black.
/// Values as used by bkbtl.
pub const PALETTES: [[u32; 4]; 16] = [
    [0x000000, 0x0000FF, 0x00FF00, 0xFF0000],
    [0x000000, 0xFFFF00, 0xFF00FF, 0xFF0000],
    [0x000000, 0x00FFFF, 0x0000FF, 0xFF00FF],
    [0x000000, 0x00FF00, 0x00FFFF, 0xFFFF00],
    [0x000000, 0xFF00FF, 0x00FFFF, 0xFFFFFF],
    [0x000000, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF],
    [0x000000, 0xC00000, 0x8E0000, 0xFF0000],
    [0x000000, 0xC0FF00, 0x8EFF00, 0xFFFF00],
    [0x000000, 0xC000FF, 0x8E00FF, 0xFF00FF],
    [0x000000, 0x8EFF00, 0x8E00FF, 0x8E0000],
    [0x000000, 0xC0FF00, 0xC000FF, 0xC00000],
    [0x000000, 0x00FFFF, 0xFFFF00, 0xFF0000],
    [0x000000, 0xFF0000, 0x00FF00, 0x00FFFF],
    [0x000000, 0x00FFFF, 0xFFFF00, 0xFFFFFF],
    [0x000000, 0xFFFF00, 0x00FF00, 0xFFFFFF],
    [0x000000, 0x00FFFF, 0x00FF00, 0xFFFFFF],
];

/// Render one screen page into `out` (SCREEN_WIDTH × SCREEN_HEIGHT).
///
/// `scroll` is the 177664 register: bits 7–0 give the line offset (0330
/// means no scroll); bit 9 clear shows only the top quarter of the screen.
pub fn render(page: &[u8], scroll: u16, palette: usize, monitor: Monitor, out: &mut [u32]) {
    assert_eq!(out.len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    let offset = usize::from(scroll & 0o377).wrapping_sub(0o330) & 0o377;
    let visible_lines = if scroll & 0o1000 != 0 {
        SCREEN_HEIGHT
    } else {
        SCREEN_HEIGHT / 4
    };
    let colors = &PALETTES[palette & 15];

    for (y, row) in out.as_chunks_mut::<SCREEN_WIDTH>().0.iter_mut().enumerate() {
        if y >= visible_lines {
            row.fill(0);
            continue;
        }
        // Rows wrap round the whole buffer in both modes; the small screen
        // only stops after 64 lines (as the MiST core's video does). The
        // BK-0010 monitor's extended memory mode sets the offset to 0230,
        // so its 64 lines are the last quarter of the buffer (BOS §1.2.4).
        let line = (y + offset) & 0o377;
        let src = &page[line * LINE_BYTES..(line + 1) * LINE_BYTES];
        for (byte, px) in src.iter().zip(row.as_chunks_mut::<8>().0) {
            match monitor {
                Monitor::Mono => {
                    for (bit, p) in px.iter_mut().enumerate() {
                        *p = if (byte >> bit) & 1 != 0 { 0xFFFFFF } else { 0 };
                    }
                }
                Monitor::Color => {
                    for (pair, p) in px.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                        p.fill(colors[usize::from((byte >> (2 * pair)) & 3)]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_order_scroll_and_quarter_screen() {
        let mut page = vec![0u8; 0o40000];
        page[0] = 0b0000_0001; // line 0: leftmost mono pixel / color 1
        page[LINE_BYTES] = 0b0000_1000; // line 1: mono pixel 3 / color 2 at pixel 1
        let mut out = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];

        render(&page, 0o1330, 0, Monitor::Mono, &mut out);
        assert_eq!(out[0], 0xFFFFFF);
        assert_eq!(out[1], 0);
        assert_eq!(out[SCREEN_WIDTH + 3], 0xFFFFFF);

        render(&page, 0o1330, 0, Monitor::Color, &mut out);
        assert_eq!(&out[..2], &[0x0000FF, 0x0000FF]);
        assert_eq!(
            &out[SCREEN_WIDTH + 2..SCREEN_WIDTH + 4],
            &[0x00FF00, 0x00FF00]
        );

        // Scrolled by one line: memory line 1 is at the top.
        render(&page, 0o1331, 0, Monitor::Mono, &mut out);
        assert_eq!(out[3], 0xFFFFFF);
        assert_eq!(
            out[255 * SCREEN_WIDTH],
            0xFFFFFF,
            "line 0 wraps to the bottom"
        );

        // Bit 9 clear: only 64 lines are shown. With the monitor's
        // extended-memory offset (0230) they are the last quarter of the
        // buffer; the offset still counts round the whole buffer.
        page[0o300 * LINE_BYTES] = 1;
        page[100 * LINE_BYTES] = 1;
        render(&page, 0o230, 0, Monitor::Mono, &mut out);
        assert_eq!(out[0], 0xFFFFFF, "line 0 shows buffer row 300");
        assert_eq!(out[100 * SCREEN_WIDTH], 0, "line 100 is blank");
        render(&page, 0o330, 0, Monitor::Mono, &mut out);
        assert_eq!(out[0], 0xFFFFFF, "offset 0: buffer row 0 at the top");
        assert_eq!(out[SCREEN_WIDTH + 3], 0xFFFFFF, "and row 1 under it");
        assert_eq!(out[100 * SCREEN_WIDTH], 0);
    }
}
