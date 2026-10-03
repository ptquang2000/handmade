pub mod debug_platform {
    use super::game::Thread;

    pub type ReadEntireFile = fn(thread: &Thread, filename: &str) -> Option<(*mut (), i64)>;
    pub type WriteEntireFile =
        fn(thread: &Thread, filename: &str, memory: *mut (), memory_size: i64) -> bool;
    pub type FreeFileMemory = fn(thread: &Thread, memory: *mut (), size: i64);
}

pub mod game {
    use super::debug_platform;

    pub type UpdateAndRender = extern "C" fn(
        thread: &Thread,
        memory: &mut Memory,
        inputs: &mut Input,
        buffer: &mut OffscreenBuffer,
    );
    const _: UpdateAndRender = update_and_render;
    pub type GetSoundSample =
        extern "C" fn(thread: &Thread, memory: &mut Memory, sound_buffer: &SoundBuffer);
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
        pub mouse_buttons: [ButtonState; 5],
        pub mouse_x: i32,
        pub mouse_y: i32,
        pub mouse_z: i32,

        pub seconds_to_advance_over_update: f32,

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

        fn read_entire_file(&self, thread: &Thread, filename: &str) -> Option<(*mut (), i64)> {
            if let Some(func) = self.read_entire_file_stub {
                func(thread, filename)
            } else {
                None
            }
        }

        fn write_entire_file(
            &self,
            thread: &Thread,
            filename: &str,
            memory: *mut (),
            memory_size: i64,
        ) -> bool {
            if let Some(func) = self.write_entire_file_stub {
                func(thread, filename, memory, memory_size)
            } else {
                false
            }
        }

        fn free_file_memory(&self, thread: &Thread, memory: *mut (), size: i64) {
            if let Some(func) = self.free_file_memory_stub {
                func(thread, memory, size)
            }
        }
    }

    #[derive(Default)]
    pub struct Thread {
        _placeholder: i32,
    }

    pub struct State {}

    #[no_mangle]
    extern "C" fn update_and_render(
        thread: &Thread,
        memory: &mut Memory,
        inputs: &mut Input,
        buffer: &mut OffscreenBuffer,
    ) {
        if !memory.is_initialized {
            memory.is_initialized = true;
        }
        let game_state = memory.get_game_state();

        for controller in &mut inputs.controllers {
            if controller.is_connected {
                if controller.is_analog {
                } else {
                }
            }
        }

        draw_rectangle(
            buffer,
            0.,
            0.,
            buffer.width as f32,
            buffer.height as f32,
            0x00FF00FF,
        );
        draw_rectangle(buffer, 10., 10., 40., 40., 0x0000FFFF);
    }

    #[no_mangle]
    extern "C" fn get_sound_samples(
        _thread: &Thread,
        memory: &mut Memory,
        sound_buffer: &SoundBuffer,
    ) {
        let game_state = memory.get_game_state();
        output_sound(game_state, sound_buffer, 480);
    }

    fn output_sound(game_state: &mut State, sound_buffer: &SoundBuffer, tone_hz: i32) {
        let tone_volume = 3000.;
        let wave_period = sound_buffer.samples_per_second as f32 / tone_hz as f32;

        for sample in sound_buffer
            .samples()
            .chunks_exact_mut(sound_buffer.bytes_per_sample as usize)
        {
            #[cfg(any())]
            let sample_value = {
                let sine_value = game_state.t_sine.sin();
                ((sine_value * tone_volume) as i16).to_be_bytes()
            };
            #[cfg(all())]
            let sample_value = (0 as i16).to_be_bytes();

            for sample_per_channel in sample.chunks_exact_mut(sample_value.len()) {
                sample_per_channel.copy_from_slice(&sample_value);
            }

            #[cfg(any())]
            {
                game_state.t_sine += 2. * std::f32::consts::PI * 1. / wave_period;
                game_state.t_sine = game_state.t_sine.rem_euclid(2. * std::f32::consts::PI);
            }
        }
    }

    fn draw_rectangle(
        buffer: &mut OffscreenBuffer,
        real_min_x: f32,
        real_min_y: f32,
        real_max_x: f32,
        real_max_y: f32,
        color: u32,
    ) {
        let min_x = (real_min_x.round() as i32).max(0);
        let min_y = (real_min_y.round() as i32).max(0);
        let max_x = (real_max_x.round() as i32).min(buffer.width);
        let max_y = (real_max_y.round() as i32).min(buffer.height);

        for rows in buffer
            .as_slice_mut()
            .chunks_exact_mut(buffer.pitch as usize)
            .take(max_y as usize)
            .skip(min_y as usize)
        {
            for pixel in rows
                .chunks_exact_mut(buffer.bytes_per_pixel as usize)
                .take(max_x as usize)
                .skip(min_x as usize)
            {
                pixel.copy_from_slice(&color.to_ne_bytes());
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
