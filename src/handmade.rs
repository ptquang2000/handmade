pub mod game {
    pub struct OffscreenBuffer<'a> {
        pub memory: &'a mut [u8],
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
        pub bytes_per_pixel: i32,
    }

    pub struct SoundOutputBuffer<'a> {
        pub samples: &'a mut [u8],
        pub samples_per_second: u32,
        pub bytes_per_sample: u32,
    }

    #[derive(Default, Copy, Clone)]
    pub struct ButtonState {
        pub half_transition_count: i32,
        pub ended_down: bool,
    }

    #[derive(Default, Copy, Clone)]
    pub struct ControllerInput {
        pub up: ButtonState,
        pub down: ButtonState,
        pub left: ButtonState,
        pub right: ButtonState,
        pub left_shoulder: ButtonState,
        pub right_shoulder: ButtonState,
    }

    #[derive(Default, Copy, Clone)]
    pub struct Input {
        pub controllers: [ControllerInput; 1],
    }

    pub fn update_and_render(inputs: &mut Input, buffer: &mut OffscreenBuffer) {
        static mut BLUE_OFFSET: i32 = 0;
        static mut GREEN_OFFSET: i32 = 0;

        let input0 = &inputs.controllers[0];
        if input0.down.ended_down {
            unsafe { GREEN_OFFSET += 4 };
        }

        render_weird_gradient(
            buffer.memory,
            buffer.pitch as usize,
            buffer.bytes_per_pixel as usize,
            unsafe { BLUE_OFFSET },
            unsafe { GREEN_OFFSET },
        );
    }

    pub fn output_sound(sound_buffer: SoundOutputBuffer, tone_hz: u32) {
        static mut T_SINE: f32 = 0.;
        let tone_volume = 0.2;
        let wave_period = sound_buffer.samples_per_second as f32 / tone_hz as f32;

        for sample in sound_buffer
            .samples
            .chunks_exact_mut(sound_buffer.bytes_per_sample as usize)
        {
            let sample_value = (unsafe { T_SINE.sin() } * tone_volume).to_ne_bytes();
            for sample_per_channel in sample.chunks_exact_mut(sample_value.len()) {
                sample_per_channel.copy_from_slice(&sample_value);
            }
            unsafe { T_SINE += 2 as f32 * std::f32::consts::PI * 1.0 / wave_period };
        }
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
