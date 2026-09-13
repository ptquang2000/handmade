pub mod game {
    pub struct OffscreenBuffer<'a> {
        pub memory: &'a mut [u8],
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
        pub bytes_per_pixel: i32,
    }

    pub fn update_and_render(buffer: &mut OffscreenBuffer, x_offset: i32, y_offset: i32) {
        render_weird_gradient(
            buffer.memory,
            buffer.pitch as usize,
            buffer.bytes_per_pixel as usize,
            x_offset,
            y_offset,
        );
    }

    fn render_weird_gradient(
        memory: &mut [u8],
        pitch: usize,
        bytes_per_pixel: usize,
        x_offset: i32,
        y_offset: i32,
    ) {
        let rows = memory.chunks_exact_mut(pitch);
        for (y, row) in rows.enumerate() {
            let pixels = row.chunks_exact_mut(bytes_per_pixel);
            for (x, pixel) in pixels.enumerate() {
                let blue = (x as i32 + x_offset) & 0xFF;
                let green = (y as i32 + y_offset) & 0xFF;
                pixel.copy_from_slice(&(blue | green << 8).to_ne_bytes());
            }
        }
    }
}
