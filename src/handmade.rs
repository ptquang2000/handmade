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

    pub struct Memory<'a> {
        pub is_initialized: bool,

        pub permanent_storage: &'a mut [u8],
        pub transient_storage: &'a mut [u8],
    }

    impl Memory<'_> {
        fn get_game_state(&self) -> &mut State {
            assert!(std::mem::size_of::<State>() <= self.transient_storage.len());
            unsafe { &mut *(self.permanent_storage.as_ptr() as *mut State) }
        }
    }

    pub struct State {
        tone_hz: i32,
        green_offset: i32,
        blue_offset: i32,
    }

    pub fn update_and_render(
        memory: &mut Memory,
        inputs: &mut Input,
        buffer: &mut OffscreenBuffer,
    ) {
        if !memory.is_initialized {
            let game_state = memory.get_game_state();
            game_state.tone_hz = 256;

            memory.is_initialized = true;
        }
        let game_state = memory.get_game_state();

        let input0 = &inputs.controllers[0];
        if input0.down.ended_down {
            game_state.green_offset += 4;
        }

        render_weird_gradient(
            buffer.memory,
            buffer.pitch as usize,
            buffer.bytes_per_pixel as usize,
            game_state.blue_offset,
            game_state.green_offset,
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
