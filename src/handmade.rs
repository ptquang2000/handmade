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

        pub dt_for_frame: f32,

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

    pub struct State {
        player_x: f32,
        player_y: f32,
    }

    #[derive(Default, Clone, Copy)]
    struct TileMap<'a> {
        count_x: i32,
        count_y: i32,

        upper_left_x: f32,
        upper_left_y: f32,
        tile_width: f32,
        tile_height: f32,

        tiles: &'a [u32],
    }

    fn get_tile_value_unchecked(tile_map: &TileMap, tile_x: i32, tile_y: i32) -> u32 {
        tile_map.tiles[(tile_y * tile_map.count_x + tile_x) as usize]
    }

    fn is_tile_map_point_empty(tile_map: &TileMap, test_x: f32, test_y: f32) -> bool {
        let player_tile_x = ((test_x - tile_map.upper_left_x) / tile_map.tile_width).floor() as i32;
        let player_tile_y =
            ((test_y - tile_map.upper_left_y) / tile_map.tile_height).floor() as i32;

        if (player_tile_x >= 0 && player_tile_x < tile_map.count_x as i32)
            && (player_tile_y >= 0 && player_tile_y < tile_map.count_y as i32)
        {
            get_tile_value_unchecked(tile_map, player_tile_x, player_tile_y) == 0
        } else {
            false
        }
    }

    #[derive(Default)]
    struct World<'a> {
        tile_map_count_x: i32,
        tile_map_count_y: i32,

        tile_maps: &'a [TileMap<'a>],
    }

    fn get_tile_map<'a>(
        world: &'a World,
        tile_map_x: i32,
        tile_map_y: i32,
    ) -> Option<&'a TileMap<'a>> {
        if (tile_map_x >= 0 && tile_map_x < world.tile_map_count_x)
            && (tile_map_y >= 0 && tile_map_y < world.tile_map_count_y)
        {
            Some(&world.tile_maps[(tile_map_y * world.tile_map_count_x + tile_map_x) as usize])
        } else {
            None
        }
    }

    fn is_world_point_empty(
        world: &World,
        tile_map_x: i32,
        tile_map_y: i32,
        test_x: f32,
        test_y: f32,
    ) -> bool {
        if let Some(tile_map) = get_tile_map(world, tile_map_x, tile_map_y) {
            let player_tile_x =
                ((test_x - tile_map.upper_left_x) / tile_map.tile_width).floor() as i32;
            let player_tile_y =
                ((test_y - tile_map.upper_left_y) / tile_map.tile_height).floor() as i32;

            if (player_tile_x >= 0 && player_tile_x < tile_map.count_x as i32)
                && (player_tile_y >= 0 && player_tile_y < tile_map.count_y as i32)
            {
                return get_tile_value_unchecked(tile_map, player_tile_x, player_tile_y) == 0;
            }
        }
        false
    }

    #[no_mangle]
    extern "C" fn update_and_render(
        thread: &Thread,
        memory: &mut Memory,
        inputs: &mut Input,
        buffer: &mut OffscreenBuffer,
    ) {
        const TILE_MAP_COUNT_X: usize = 17;
        const TILE_MAP_COUNT_Y: usize = 9;
        let tiles00: [[u32; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
            [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1],
            [1, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1],
            [1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
            [1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
        ];
        let tiles01: [[u32; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
            [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        ];
        let tiles10: [[u32; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
            [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
        ];
        let tiles11: [[u32; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
            [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        ];

        let mut tile_maps = [[TileMap::default(); 2]; 2];
        tile_maps[0][0].count_x = TILE_MAP_COUNT_X as i32;
        tile_maps[0][0].count_y = TILE_MAP_COUNT_Y as i32;

        tile_maps[0][0].upper_left_x = -30.;
        tile_maps[0][0].upper_left_y = 0.;
        tile_maps[0][0].tile_width = 60.;
        tile_maps[0][0].tile_height = 60.;

        tile_maps[0][0].tiles = tiles00.as_flattened();

        tile_maps[0][1] = tile_maps[0][0];
        tile_maps[0][1].tiles = tiles01.as_flattened();

        tile_maps[0][1] = tile_maps[0][0];
        tile_maps[1][0].tiles = tiles10.as_flattened();

        tile_maps[0][1] = tile_maps[0][0];
        tile_maps[1][1].tiles = tiles11.as_flattened();

        let tile_map = &tile_maps[0][0];

        let mut world = World::default();
        world.tile_map_count_x = 2;
        world.tile_map_count_y = 2;

        world.tile_maps = tile_maps.as_flattened();

        let player_width = 0.75 * tile_map.tile_width;
        let player_height = tile_map.tile_height;

        if !memory.is_initialized {
            let game_state = memory.get_game_state();

            game_state.player_x = 150.;
            game_state.player_y = 150.;

            memory.is_initialized = true;
        }

        let game_state = memory.get_game_state();

        for controller in &mut inputs.controllers {
            if controller.is_connected {
                if controller.is_analog {
                } else {
                    let mut dplayer_x = 0.;
                    let mut dplayer_y = 0.;

                    if controller.move_up().ended_down {
                        dplayer_y -= 1.;
                    }

                    if controller.move_down().ended_down {
                        dplayer_y += 1.;
                    }

                    if controller.move_left().ended_down {
                        dplayer_x -= 1.;
                    }

                    if controller.move_right().ended_down {
                        dplayer_x += 1.;
                    }
                    dplayer_x *= 64.;
                    dplayer_y *= 64.;

                    let new_player_x = game_state.player_x + inputs.dt_for_frame * dplayer_x;
                    let new_player_y = game_state.player_y + inputs.dt_for_frame * dplayer_y;

                    if is_tile_map_point_empty(
                        &tile_map,
                        new_player_x - 0.5 * player_width,
                        new_player_y,
                    ) && is_tile_map_point_empty(
                        &tile_map,
                        new_player_x + 0.5 * player_width,
                        new_player_y,
                    ) && is_tile_map_point_empty(&tile_map, new_player_x, new_player_y)
                    {
                        game_state.player_x = new_player_x;
                        game_state.player_y = new_player_y;
                    }
                }
            }
        }

        draw_rectangle(
            buffer,
            0.0,
            0.0,
            buffer.width as f32,
            buffer.height as f32,
            1.0,
            0.0,
            0.1,
        );
        for (row, columns) in tile_map
            .tiles
            .chunks_exact(tile_map.count_x as usize)
            .enumerate()
        {
            for (column, _) in columns.iter().enumerate() {
                let gray = if get_tile_value_unchecked(tile_map, column as i32, row as i32) == 1 {
                    1.0
                } else {
                    0.5
                };
                let min_x = tile_map.upper_left_x + column as f32 * tile_map.tile_width;
                let min_y = tile_map.upper_left_y + row as f32 * tile_map.tile_height;
                let max_x = min_x + tile_map.tile_width;
                let max_y = min_y + tile_map.tile_width;
                draw_rectangle(buffer, min_x, min_y, max_x, max_y, gray, gray, gray);
            }
        }

        let player_r = 1.0;
        let player_g = 1.0;
        let player_b = 0.0;
        let player_left = game_state.player_x - 0.5 * player_width;
        let player_top = game_state.player_y - player_height;
        draw_rectangle(
            buffer,
            player_left,
            player_top,
            player_left + player_width,
            player_top + player_height,
            player_r,
            player_g,
            player_b,
        );
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
        r: f32,
        g: f32,
        b: f32,
    ) {
        let min_x = (real_min_x.round() as i32).max(0);
        let min_y = (real_min_y.round() as i32).max(0);
        let max_x = (real_max_x.round() as i32).min(buffer.width);
        let max_y = (real_max_y.round() as i32).min(buffer.height);

        let color = ((r * u8::MAX as f32).round() as u32) << 16
            | ((g * u8::MAX as f32).round() as u32) << 8
            | ((b * u8::MAX as f32).round() as u32);

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
