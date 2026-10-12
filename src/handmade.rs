include!("handmade_intrinsics.rs");

pub mod debug_platform {
    use super::game::Thread;

    pub type ReadEntireFile = fn(thread: &Thread, filename: &str) -> Option<(*mut (), i64)>;
    pub type WriteEntireFile =
        fn(thread: &Thread, filename: &str, memory: *mut (), memory_size: i64) -> bool;
    pub type FreeFileMemory = fn(thread: &Thread, memory: *mut (), size: i64);
}

pub mod game {
    use super::debug_platform;
    use super::intrinsics::*;

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
        player_p: WorldPosition,
    }

    #[derive(Default, Clone, Copy)]
    struct TileChunk<'a> {
        tiles: &'a [u32],
    }

    fn get_tile_value_unchecked(
        world: &World,
        tile_chunk: &TileChunk,
        tile_x: u32,
        tile_y: u32,
    ) -> u32 {
        debug_assert!(tile_x < world.chunk_dim);
        debug_assert!(tile_y < world.chunk_dim);
        let tile_map_value = tile_chunk.tiles[(tile_y * world.chunk_dim + tile_x) as usize];
        tile_map_value
    }

    fn get_tile_value_(
        world: &World,
        tile_chunk: Option<&TileChunk>,
        test_x: u32,
        test_y: u32,
    ) -> u32 {
        let mut tile_map_value = 0;
        if let Some(tile_map) = tile_chunk {
            tile_map_value = get_tile_value_unchecked(world, tile_map, test_x, test_y);
        }
        tile_map_value
    }

    #[derive(Default)]
    struct World<'a> {
        chunk_shift: u32,
        chunk_mask: u32,
        chunk_dim: u32,

        tile_side_in_meters: f32,
        tile_side_in_pixels: i32,
        meters_to_pixels: f32,

        tile_chunk_count_x: i32,
        tile_map_count_y: i32,

        tile_chunks: &'a [TileChunk<'a>],
    }

    fn get_tile_chunk<'a>(
        world: &'a World,
        tile_chunk_x: i32,
        tile_chunk_y: i32,
    ) -> Option<&'a TileChunk<'a>> {
        let tile_chunk = if (tile_chunk_x >= 0 && tile_chunk_x < world.tile_chunk_count_x)
            && (tile_chunk_y >= 0 && tile_chunk_y < world.tile_map_count_y)
        {
            Some(
                &world.tile_chunks
                    [(tile_chunk_y * world.tile_chunk_count_x + tile_chunk_x) as usize],
            )
        } else {
            None
        };
        tile_chunk
    }

    fn get_chunk_position_for(
        world: &World,
        abs_tile_x: u32,
        abs_tile_y: u32,
    ) -> TileChunkPosition {
        let mut result = TileChunkPosition::default();

        result.tile_chunk_x = abs_tile_x >> world.chunk_shift;
        result.tile_chunk_x = abs_tile_y >> world.chunk_shift;
        result.rel_tile_x = abs_tile_x & world.chunk_mask;
        result.rel_tile_y = abs_tile_y & world.chunk_mask;

        result
    }

    fn get_tile_value(world: &World, abs_tile_x: u32, abs_tile_y: u32) -> u32 {
        let chunk_pos = get_chunk_position_for(world, abs_tile_x, abs_tile_y);
        let tile_map = get_tile_chunk(
            world,
            chunk_pos.tile_chunk_x as i32,
            chunk_pos.tile_chunk_y as i32,
        );
        let tile_chunk_value =
            get_tile_value_(world, tile_map, chunk_pos.rel_tile_x, chunk_pos.rel_tile_y);
        return tile_chunk_value;
    }

    fn is_world_point_empty(world: &World, can_pos: WorldPosition) -> bool {
        let tile_chunk_value =
            get_tile_value(world, can_pos.abs_tile_x as u32, can_pos.abs_tile_y as u32);
        let empty = tile_chunk_value == 0;

        empty
    }

    #[derive(Default, Clone, Copy)]
    struct TileChunkPosition {
        tile_chunk_x: u32,
        tile_chunk_y: u32,

        rel_tile_x: u32,
        rel_tile_y: u32,
    }

    #[derive(Default, Clone, Copy)]
    struct WorldPosition {
        abs_tile_x: i32,
        abs_tile_y: i32,

        tile_rel_x: f32,
        tile_rel_y: f32,
    }

    #[derive(Default, Clone, Copy)]
    struct RawPosition {
        tile_map_x: i32,
        tile_map_y: i32,

        x: f32,
        y: f32,
    }

    fn recanonicalize_coord(world: &World, tile: &mut i32, tile_rel: &mut f32) {
        let offset = floor_f32_to_i32(*tile_rel / world.tile_side_in_meters as f32);
        *tile += offset;
        *tile_rel -= offset as f32 * world.tile_side_in_meters;

        debug_assert!(*tile_rel >= 0.);
        debug_assert!(*tile_rel <= world.tile_side_in_meters as f32);
    }

    fn recanonicalize_position(world: &World, pos: WorldPosition) -> WorldPosition {
        let mut result = pos;

        recanonicalize_coord(world, &mut result.abs_tile_x, &mut result.tile_rel_x);
        recanonicalize_coord(world, &mut result.abs_tile_y, &mut result.tile_rel_y);

        result
    }

    #[no_mangle]
    extern "C" fn update_and_render(
        thread: &Thread,
        memory: &mut Memory,
        inputs: &mut Input,
        buffer: &mut OffscreenBuffer,
    ) {
        const TILE_MAP_COUNT_X: usize = 256;
        const TILE_MAP_COUNT_Y: usize = 256;
        const TILE_MAP: [[u32; 34]; 18] = [
            [
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                1, 1, 1, 1, 1, 1,
            ],
            [
                1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1,
                1, 1, 1, 1, 1, 1,
            ],
            [
                1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1,
                1, 1, 1, 1, 1, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 1,
            ],
            [
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                1, 1, 1, 1, 1, 1,
            ],
        ];
        let mut temp_tiles = [[0u32; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y];
        for (dst, src) in temp_tiles.iter_mut().zip(TILE_MAP) {
            dst[..src.len()].copy_from_slice(&src);
        }

        let mut world = World::default();
        world.chunk_shift = 8;
        world.chunk_mask = (1 << world.chunk_shift) - 1;
        world.chunk_dim = 256;

        world.tile_chunk_count_x = 1;
        world.tile_map_count_y = 1;

        world.tile_side_in_meters = 1.4;
        world.tile_side_in_pixels = 60;
        world.meters_to_pixels = world.tile_side_in_pixels as f32 / world.tile_side_in_meters;

        let player_height = 1.4;
        let player_width = 0.75 * player_height;

        let lower_left_x = -world.tile_side_in_pixels as f32 / 2.;
        let lower_left_y = buffer.height as f32;

        let mut tile_chunks = [TileChunk::default()];
        tile_chunks[0].tiles = temp_tiles.as_flattened();
        world.tile_chunks = &tile_chunks;

        if !memory.is_initialized {
            let game_state = memory.get_game_state();

            game_state.player_p.abs_tile_x = 3;
            game_state.player_p.abs_tile_y = 3;
            game_state.player_p.tile_rel_x = 5.;
            game_state.player_p.tile_rel_y = 5.;

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
                        dplayer_y = 1.;
                    }

                    if controller.move_down().ended_down {
                        dplayer_y = -1.;
                    }

                    if controller.move_left().ended_down {
                        dplayer_x = -1.;
                    }

                    if controller.move_right().ended_down {
                        dplayer_x = 1.;
                    }
                    dplayer_x *= 2.;
                    dplayer_y *= 2.;

                    let mut new_player_p = game_state.player_p;
                    new_player_p.tile_rel_x += inputs.dt_for_frame * dplayer_x;
                    new_player_p.tile_rel_y += inputs.dt_for_frame * dplayer_y;
                    new_player_p = recanonicalize_position(&world, new_player_p);

                    let mut player_left = new_player_p;
                    player_left.tile_rel_x -= 0.5 * player_width;
                    player_left = recanonicalize_position(&world, player_left);

                    let mut player_right = new_player_p;
                    player_right.tile_rel_x += 0.5 * player_width;
                    player_right = recanonicalize_position(&world, player_right);

                    if is_world_point_empty(&world, new_player_p)
                        && is_world_point_empty(&world, player_left)
                        && is_world_point_empty(&world, player_right)
                    {
                        game_state.player_p = new_player_p;
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

        let center_x = 0.5 * buffer.width as f32;
        let center_y = 0.5 * buffer.height as f32;

        for rel_row in -10..10 {
            for rel_column in -20..20 {
                let column =
                    (game_state.player_p.abs_tile_x as u32).wrapping_add(rel_column as u32);
                let row = (game_state.player_p.abs_tile_y as u32).wrapping_add(rel_row as u32);
                let tile_id = get_tile_value(&world, column, row);
                let mut gray = 0.5;
                if tile_id == 1 {
                    gray = 1.0;
                }

                if column as i32 == game_state.player_p.abs_tile_x
                    && row as i32 == game_state.player_p.abs_tile_y
                {
                    gray = 0.
                };

                let min_x = center_x + rel_column as f32 * world.tile_side_in_pixels as f32;
                let min_y = center_y - rel_row as f32 * world.tile_side_in_pixels as f32;
                let max_x = min_x + world.tile_side_in_pixels as f32;
                let max_y = min_y - world.tile_side_in_pixels as f32;
                draw_rectangle(buffer, min_x, max_y, max_x, min_y, gray, gray, gray);
            }
        }

        let player_r = 1.0;
        let player_g = 1.0;
        let player_b = 0.0;
        let player_left = center_x + world.meters_to_pixels * game_state.player_p.tile_rel_x
            - 0.5 * world.meters_to_pixels * player_width;
        let player_top = center_y
            - world.meters_to_pixels * game_state.player_p.tile_rel_y
            - world.meters_to_pixels * player_height;
        draw_rectangle(
            buffer,
            player_left,
            player_top,
            player_left + world.meters_to_pixels * player_width,
            player_top + world.meters_to_pixels * player_height,
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
        let max_x = round_f32_to_i32(real_max_x).min(buffer.width);
        let max_y = round_f32_to_i32(real_max_y).min(buffer.height);
        let min_x = round_f32_to_i32(real_min_x).max(0).min(max_x);
        let min_y = round_f32_to_i32(real_min_y).max(0).min(max_y);

        let color = round_f32_to_u32(r * u8::MAX as f32) << 16
            | round_f32_to_u32(g * u8::MAX as f32) << 8
            | round_f32_to_u32(b * u8::MAX as f32);

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
