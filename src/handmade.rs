pub mod debug_platform {
    pub type ReadEntireFile = fn(filename: &str) -> Option<(*mut (), i64)>;
    pub type WriteEntireFile = fn(filename: &str, memory: *mut (), memory_size: i64) -> bool;
    pub type FreeFileMemory = fn(memory: *mut (), size: i64);
}

pub mod game {
    use super::debug_platform;

    pub type UpdateAndRender =
        extern "C" fn(memory: &mut Memory, inputs: Input, buffer: OffscreenBuffer);
    const _: UpdateAndRender = update_and_render;
    pub type GetSoundSample = extern "C" fn(memory: &mut Memory, sound_buffer: &SoundBuffer);
    const _: GetSoundSample = get_sound_samples;

    #[repr(C)]
    #[derive(Default)]
    pub struct OffscreenBuffer {
        pub memory: *mut u8,
        pub bytes_per_pixel: i32,
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
    }

    impl OffscreenBuffer {
        fn as_slice_mut(&self) -> &mut [u8] {
            let size = self.pitch * self.height;
            unsafe { std::slice::from_raw_parts_mut(self.memory, size as usize) }
        }
    }

    #[derive(Default)]
    pub struct SoundBuffer {
        pub samples_per_second: i32,
        pub bytes_per_sample: i32,
        pub sample_count: i32,
        pub memory: *mut u8,
    }

    impl SoundBuffer {
        fn samples(&self) -> &mut [u8] {
            let size = self.sample_count * self.bytes_per_sample;
            unsafe { std::slice::from_raw_parts_mut(self.memory, size as usize) }
        }
    }

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    pub struct ButtonState {
        pub half_transition_count: i32,
        pub ended_down: bool,
    }

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    pub struct ControllerInput {
        pub is_connected: bool,
        pub is_analog: bool,
        pub stick_average_x: f32,
        pub stick_average_y: f32,
        pub buttons: [ButtonState; 12],
    }

    impl ControllerInput {
        const MOVE_UP: usize = 0;
        const MOVE_DOWN: usize = 1;
        const MOVE_LEFT: usize = 2;
        const MOVE_RIGHT: usize = 3;
        const ACTION_UP: usize = 4;
        const ACTION_DOWN: usize = 5;
        const ACTION_LEFT: usize = 6;
        const ACTION_RIGHT: usize = 7;
        const LEFT_SHOULDER: usize = 8;
        const RIGHT_SHOULDER: usize = 9;
        const START: usize = 10;
        const BACK: usize = 11;

        pub fn move_up(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_UP]
        }
        pub fn move_down(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_DOWN]
        }
        pub fn move_left(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_LEFT]
        }
        pub fn move_right(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_RIGHT]
        }
        pub fn action_up(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_UP]
        }
        pub fn action_down(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_DOWN]
        }
        pub fn action_left(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_LEFT]
        }
        pub fn action_right(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_RIGHT]
        }
        pub fn left_shoulder(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::LEFT_SHOULDER]
        }
        pub fn right_shoulder(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::RIGHT_SHOULDER]
        }
        pub fn start(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::START]
        }
        pub fn back(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::BACK]
        }
    }

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    pub struct Input {
        pub controllers: [ControllerInput; 5],
    }

    #[derive(Default)]
    pub struct Memory {
        pub is_initialized: bool,

        pub permanent_storage_size: usize,
        pub permanent_storage_memory: *mut u8,

        pub transient_storage_size: usize,
        pub transient_storage_memory: *mut u8,

        pub read_entire_file_stub: Option<debug_platform::ReadEntireFile>,
        pub write_entire_file_stub: Option<debug_platform::WriteEntireFile>,
        pub free_file_memory_stub: Option<debug_platform::FreeFileMemory>,
    }

    impl Memory {
        fn get_game_state(&self) -> &mut State {
            assert!(std::mem::size_of::<State>() <= self.transient_storage_size as usize);
            unsafe { &mut *(self.permanent_storage_memory as *mut State) }
        }

        fn read_entire_file(&self, filename: &str) -> Option<(*mut (), i64)> {
            if let Some(func) = self.read_entire_file_stub {
                func(filename)
            } else {
                None
            }
        }

        fn write_entire_file(&self, filename: &str, memory: *mut (), memory_size: i64) -> bool {
            if let Some(func) = self.write_entire_file_stub {
                func(filename, memory, memory_size)
            } else {
                false
            }
        }

        fn free_file_memory(&self, memory: *mut (), size: i64) {
            if let Some(func) = self.free_file_memory_stub {
                func(memory, size)
            }
        }
    }

    pub struct State {
        tone_hz: i32,
        green_offset: i32,
        blue_offset: i32,
        t_sine: f32,

        player_x: i32,
        player_y: i32,
        t_jump: f32,
    }

    #[no_mangle]
    extern "C" fn update_and_render(
        memory: &mut Memory,
        mut inputs: Input,
        mut buffer: OffscreenBuffer,
    ) {
        if !memory.is_initialized {
            let game_state = memory.get_game_state();

            let filename = concat!(file!(), "\0");
            if let Some((contents, contents_size)) = memory.read_entire_file(filename) {
                memory.write_entire_file("test.out\0", contents, contents_size);
                memory.free_file_memory(contents, contents_size);
            }

            game_state.tone_hz = 512;
            game_state.t_sine = 0.;

            game_state.player_x = 100;
            game_state.player_y = 100;

            memory.is_initialized = true;
        }
        let game_state = memory.get_game_state();

        for controller in &mut inputs.controllers {
            if controller.is_connected {
                if controller.is_analog {
                    game_state.blue_offset += (4. * controller.stick_average_x) as i32;
                    game_state.tone_hz =
                        512 + (-(i8::MIN as f32) * controller.stick_average_y) as i32;
                } else {
                    if controller.move_left().ended_down {
                        game_state.blue_offset -= 1;
                    }
                    if controller.move_right().ended_down {
                        game_state.blue_offset += 1;
                    }
                }

                game_state.player_x += (4. * controller.stick_average_x) as i32;
                game_state.player_y -= (4. * controller.stick_average_y) as i32;
                if game_state.t_jump > 0. {
                    game_state.player_y +=
                        (10. * (0.5 * std::f32::consts::PI * game_state.t_jump).sin()) as i32;
                }
                if controller.action_down().ended_down {
                    game_state.t_jump = 4.;
                }
                game_state.t_jump -= 0.033;
            }
        }

        render_weird_gradient(&mut buffer, game_state.blue_offset, game_state.green_offset);
        render_player(&mut buffer, game_state.player_x, game_state.player_y);
    }

    #[no_mangle]
    extern "C" fn get_sound_samples(memory: &mut Memory, sound_buffer: &SoundBuffer) {
        let game_state = memory.get_game_state();
        output_sound(game_state, sound_buffer, game_state.tone_hz);
    }

    fn output_sound(game_state: &mut State, sound_buffer: &SoundBuffer, tone_hz: i32) {
        let tone_volume = 3000.;
        let wave_period = sound_buffer.samples_per_second as f32 / tone_hz as f32;

        for sample in sound_buffer
            .samples()
            .chunks_exact_mut(sound_buffer.bytes_per_sample as usize)
        {
            let sine_value = if cfg!(all()) {
                game_state.t_sine.sin()
            } else {
                0.
            };
            let sample_value = ((sine_value * tone_volume) as i16).to_be_bytes();
            for sample_per_channel in sample.chunks_exact_mut(sample_value.len()) {
                sample_per_channel.copy_from_slice(&sample_value);
            }

            game_state.t_sine += 2. * std::f32::consts::PI * 1. / wave_period;
            game_state.t_sine = game_state.t_sine.rem_euclid(2. * std::f32::consts::PI);
        }
    }

    fn render_player(buffer: &mut OffscreenBuffer, player_x: i32, player_y: i32) {
        let color = 0xFFFFFFFFu32;
        let top = player_y;
        let bottom = player_y + 10;
        for x in player_x..player_x + 10 {
            let rows = buffer
                .as_slice_mut()
                .chunks_exact_mut(buffer.pitch as usize)
                .skip(top as usize)
                .take((bottom - top) as usize);
            for row in rows {
                if let Some(pixel) = row
                    .chunks_exact_mut(buffer.bytes_per_pixel as usize)
                    .nth(x as usize)
                {
                    pixel.copy_from_slice(&color.to_ne_bytes());
                }
            }
        }
    }

    fn render_weird_gradient(buffer: &mut OffscreenBuffer, x_offset: i32, y_offset: i32) {
        let rows = buffer
            .as_slice_mut()
            .chunks_exact_mut(buffer.pitch as usize);
        for (y, row) in rows.enumerate() {
            let pixels = row.chunks_exact_mut(buffer.bytes_per_pixel as usize);
            for (x, pixel) in pixels.enumerate() {
                let blue = (x as i32 + x_offset) & 0xFF;
                let green = (y as i32 + y_offset) & 0xFF;
                pixel.copy_from_slice(&(green << 8 | blue).to_ne_bytes());
            }
        }
    }
}
