#[cfg(not(HANDMADE_INTERNAL))]
pub mod debug_platform {
    pub fn read_entire_file(filename: &str) -> Option<(*mut (), i64)> {
        None
    }

    pub fn write_entire_file(filename: &str, memory: *mut (), memory_size: i64) -> bool {
        false
    }

    pub fn free_file_memory(memory: *mut (), size: i64) {}
}

pub mod game {
    use crate::debug_platform;

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

    #[derive(Default, Copy, Clone)]
    pub struct Input {
        pub controllers: [ControllerInput; 5],
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

    pub fn update_and_render(memory: &mut Memory, mut inputs: Input, buffer: OffscreenBuffer) {
        if !memory.is_initialized {
            let game_state = memory.get_game_state();
            game_state.tone_hz = 256;

            let filename = concat!(file!(), "\0");
            if let Some((contents, contents_size)) = debug_platform::read_entire_file(filename) {
                debug_platform::write_entire_file("test.out\0", contents, contents_size);
                debug_platform::free_file_memory(contents, contents_size);
            }

            memory.is_initialized = true;
        }
        let game_state = memory.get_game_state();

        for controller in &mut inputs.controllers {
            if controller.is_connected {
                if controller.is_analog {
                    game_state.blue_offset += (4. * controller.stick_average_x) as i32;
                } else {
                    if controller.move_left().ended_down {
                        game_state.blue_offset -= 1;
                    } else if controller.move_right().ended_down {
                        game_state.blue_offset += 1;
                    }
                }

                if controller.action_down().ended_down {
                    game_state.green_offset += 1;
                }
            }
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
