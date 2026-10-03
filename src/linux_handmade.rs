mod handmade;

macro_rules! kilobytes {
    ($value:expr) => {
        $value * 1024
    };
}
macro_rules! megabytes {
    ($value:expr) => {
        kilobytes!($value) * 1024
    };
}
macro_rules! gigabytes {
    ($value:expr) => {
        megabytes!($value) * 1024
    };
}
macro_rules! terabytes {
    ($value:expr) => {
        gigabytes!($value) * 1024
    };
}

#[cfg(HANDMADE_INTERNAL)]
mod debug_platform {
    use crate::{handmade, linux};

    pub fn read_entire_file(
        _thread: &handmade::game::Thread,
        filename: &str,
    ) -> Option<(*mut (), i64)> {
        debug_assert!(filename.ends_with('\0'));
        unsafe {
            let fd = linux::open(
                filename.as_ptr() as *const _,
                linux::O_RDONLY,
                linux::S_IRUSR | linux::S_IRGRP | linux::S_IROTH,
            );
            if fd != -1 {
                let size = linux::lseek(fd, 0, linux::SEEK_END);
                if size != -1 {
                    let result = linux::mmap(
                        std::ptr::null_mut(),
                        size as usize,
                        linux::PROT_READ,
                        linux::MAP_PRIVATE,
                        fd,
                        0,
                    ) as *mut ();
                    if result as i32 != linux::MAP_FAILED {
                        return Some((result, size));
                    }
                } else {
                }
                linux::close(fd);
            } else {
            }
        }
        None
    }

    pub fn write_entire_file(
        _thread: &handmade::game::Thread,
        filename: &str,
        memory: *mut (),
        memory_size: i64,
    ) -> bool {
        debug_assert!(filename.ends_with('\0'));
        unsafe {
            let fd = linux::open(
                filename.as_ptr() as *const _,
                linux::O_RDWR | linux::O_CREAT | linux::O_TRUNC,
                linux::S_IRUSR | linux::S_IWUSR | linux::S_IRGRP | linux::S_IROTH,
            );
            if fd != -1 {
                if linux::ftruncate(fd, memory_size) != -1 {
                    let result = linux::mmap(
                        std::ptr::null_mut(),
                        memory_size as usize,
                        linux::PROT_WRITE,
                        linux::MAP_SHARED,
                        fd,
                        0,
                    ) as *mut ();
                    if result as i32 != linux::MAP_FAILED {
                        std::ptr::copy_nonoverlapping(
                            memory as *const u8,
                            result as *mut u8,
                            memory_size as usize,
                        );
                        linux::munmap(result as *mut std::ffi::c_void, memory_size as usize);
                        return true;
                    } else {
                    }
                }
                linux::close(fd);
            } else {
            }
        }
        false
    }

    pub fn free_file_memory(_thread: &handmade::game::Thread, memory: *mut (), size: i64) {
        unsafe { linux::munmap(memory as *mut std::ffi::c_void, size as usize) };
    }
}

mod linux {
    use crate::{handmade, libevdev, pw, wl, wp_alpha, xdg};

    pub enum KeyCode {
        ESC = 1,
        Q = 16,
        W = 17,
        E = 18,
        P = 25,
        A = 30,
        S = 31,
        D = 32,
        L = 38,
        SPACE = 57,
        UP = 103,
        LEFT = 105,
        RIGHT = 106,
        DOWN = 108,
    }

    pub enum MouseBtn {
        LEFT = 0x110,
        RIGHT = 0x111,
        MIDDLE = 0x112,
        SIDE = 0x113,
        EXTRA = 0x114,
    }

    #[derive(Default)]
    pub struct GlobalState {
        pub compositor: *mut wl::wl_compositor,
        pub shm: *mut wl::wl_shm,
        pub surface: *mut wl::wl_surface,
        pub buffer: *mut wl::wl_buffer,
        pub output: *mut wl::wl_output,

        pub seat: *mut wl::wl_seat,
        pub keyboard: *mut wl::wl_keyboard,
        pub pointer: *mut wl::wl_pointer,

        pub window_manager: *mut xdg::xdg_wm_base,
        pub window: *mut xdg::xdg_surface,
        pub toplevel: *mut xdg::xdg_toplevel,

        pub alpha: *mut wp_alpha::wp_alpha_modifier_v1,
        pub alpha_surface: *mut wp_alpha::wp_alpha_modifier_surface_v1,

        pub game_input: *mut handmade::game::Input,

        pub running: bool,
        pub pause: bool,
        pub buffer_released: std::sync::atomic::AtomicBool,
        pub back_buffer: OffscreenBuffer,

        pub linux_state: *mut State,
        pub refresh_rate: i32,
    }

    #[derive(Default)]
    pub struct SoundOutput {
        pub samples_per_second: i32,
        pub bytes_per_sample: i32,
        pub channels: i32,
        pub safety_bytes: i32,

        pub sound_main_loop: *mut pw::pw_thread_loop,
        pub sound_loop: *mut pw::pw_loop,
        pub stream: *mut pw::pw_stream,
        pub ring: pw::spa_ringbuffer,
        pub eventfd: i32,
        pub secondary_buffer: MemFd,
    }

    pub type ListenerImplementation = unsafe extern "C" fn();

    #[derive(Default)]
    pub struct OffscreenBuffer {
        pub memory: MemFd,
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
        pub bytes_per_pixel: i32,
    }

    #[derive(Default, Clone, Copy)]
    pub struct DebugTimeMarker {
        pub output_play_cursor: u32,
        pub output_write_cursor: u32,
        pub output_location: u32,
        pub output_byte_count: u32,
        pub expected_flip_play_cursor: u32,

        pub flip_play_cursor: u32,
        pub flip_write_cursor: u32,
    }

    pub fn resize_shared_buffer(global_state: &mut GlobalState, width: i32, height: i32) {
        let buffer = &mut global_state.back_buffer;
        buffer.width = width;
        buffer.height = height;
        buffer.pitch = width * buffer.bytes_per_pixel;
        let bitmap_size = height * buffer.pitch;
        if !buffer.memory.is_null() {
            memfd_release(&mut buffer.memory);
        }

        buffer.memory = memfd_alloc("handmade_hero\0", bitmap_size as i64, 0).unwrap();
        let pool = wl::shm_create_pool(global_state.shm, buffer.memory.fd, bitmap_size);
        global_state.buffer = wl::shm_pool_create_buffer(
            pool,
            0,
            buffer.width,
            buffer.height,
            buffer.pitch,
            wl::SHMFormat::XRGB8888 as u32,
        );
        wl::shm_pool_destroy(pool);
        wl::buffer_add_listener(global_state.buffer, global_state);
    }

    pub fn display_buffer_in_window(global_state: &mut GlobalState, x: i32, y: i32) {
        global_state
            .buffer_released
            .store(false, std::sync::atomic::Ordering::Release);
        wl::surface_damage_buffer(
            global_state.surface,
            x,
            y,
            global_state.back_buffer.width,
            global_state.back_buffer.height,
        );
        wl::surface_attach(global_state.surface, global_state.buffer, x, y);
        wl::surface_commit(global_state.surface);
    }

    pub fn process_keyboard_message(new_state: &mut handmade::game::ButtonState, is_down: bool) {
        if new_state.ended_down != is_down {
            new_state.ended_down = is_down;
            new_state.half_transition_count += 1;
        }
    }

    pub fn process_input_digital_button(
        old_state: &handmade::game::ButtonState,
        value: i32,
        new_state: &mut handmade::game::ButtonState,
    ) {
        new_state.ended_down = value == 1;
        new_state.half_transition_count = (old_state.ended_down != new_state.ended_down) as i32;
    }

    pub fn process_input_stick_value(controller: *mut libevdev::libevdev, code: u32) -> f32 {
        let deadzone = libevdev::get_controller_absinfo(controller, code) as i32;
        let value = libevdev::get_controller_value(controller, libevdev::EV_ABS, code);
        if value < -deadzone {
            value as f32 / -(i16::MIN as f32)
        } else if value > deadzone {
            value as f32 / (i16::MAX as f32)
        } else {
            0.
        }
    }

    pub fn get_seconds_elapsed(last_counter: timespec, work_counter: timespec) -> f64 {
        work_counter.tv_sec as f64 - last_counter.tv_sec as f64 + work_counter.tv_nsec as f64 / 1e9
            - last_counter.tv_nsec as f64 / 1e9
    }

    pub fn debug_sync_display(
        back_buffer: &mut OffscreenBuffer,
        sound_output: &SoundOutput,
        current_marker_index: usize,
        debug_time_markers: &[DebugTimeMarker],
        _target_second_per_frame: f64,
    ) {
        let pad_x = 16;
        let pad_y = 16;

        let line_height = 64;

        let c = (back_buffer.width - 2 * pad_x) as f32 / sound_output.secondary_buffer.size as f32;
        for (marker_index, &debug_time_marker) in debug_time_markers.iter().enumerate() {
            let play_color = 0xFFFFFFFF;
            let write_color = 0xFFFF0000;
            let expected_flip_color = 0xFFFFFF00;
            let play_window_color = 0xFFFF00FF;

            let mut top = pad_y;
            let mut bottom = pad_y + line_height;
            if marker_index == current_marker_index {
                debug_assert!(
                    debug_time_marker.output_play_cursor
                        < sound_output.secondary_buffer.size as u32
                );
                debug_assert!(
                    debug_time_marker.output_write_cursor
                        < sound_output.secondary_buffer.size as u32
                );
                debug_assert!(
                    debug_time_marker.output_location < sound_output.secondary_buffer.size as u32
                );
                debug_assert!(
                    debug_time_marker.output_byte_count < sound_output.secondary_buffer.size as u32
                );
                debug_assert!(
                    debug_time_marker.flip_play_cursor < sound_output.secondary_buffer.size as u32
                );
                debug_assert!(
                    debug_time_marker.flip_write_cursor < sound_output.secondary_buffer.size as u32
                );

                top += line_height + pad_y;
                bottom += line_height + pad_y;

                let first_top = top;

                debug_draw_sound_buffer_marker(
                    back_buffer,
                    c,
                    pad_x,
                    top,
                    bottom,
                    debug_time_marker.output_play_cursor,
                    play_color,
                );
                debug_draw_sound_buffer_marker(
                    back_buffer,
                    c,
                    pad_x,
                    top,
                    bottom,
                    debug_time_marker.output_write_cursor,
                    write_color,
                );

                top += line_height + pad_y;
                bottom += line_height + pad_y;

                debug_draw_sound_buffer_marker(
                    back_buffer,
                    c,
                    pad_x,
                    top,
                    bottom,
                    debug_time_marker.output_location,
                    play_color,
                );
                debug_draw_sound_buffer_marker(
                    back_buffer,
                    c,
                    pad_x,
                    top,
                    bottom,
                    debug_time_marker.output_location + debug_time_marker.output_byte_count,
                    write_color,
                );

                top += line_height + pad_y;
                bottom += line_height + pad_y;

                debug_draw_sound_buffer_marker(
                    back_buffer,
                    c,
                    pad_x,
                    first_top,
                    bottom,
                    debug_time_marker.expected_flip_play_cursor,
                    expected_flip_color,
                );
            }

            debug_draw_sound_buffer_marker(
                back_buffer,
                c,
                pad_x,
                top,
                bottom,
                debug_time_marker.flip_play_cursor,
                play_color,
            );
            debug_draw_sound_buffer_marker(
                back_buffer,
                c,
                pad_x,
                top,
                bottom,
                debug_time_marker.flip_play_cursor + 256 * sound_output.bytes_per_sample as u32,
                play_window_color,
            );
            debug_draw_sound_buffer_marker(
                back_buffer,
                c,
                pad_x,
                top,
                bottom,
                debug_time_marker.flip_write_cursor,
                write_color,
            );
        }
    }

    fn debug_draw_sound_buffer_marker(
        back_buffer: &mut OffscreenBuffer,
        c: f32,
        pad_x: i32,
        top: i32,
        bottom: i32,
        value: u32,
        color: u32,
    ) {
        let x_f32 = c * value as f32;
        let x = pad_x + x_f32 as i32;
        debug_draw_vertical(back_buffer, x, top, bottom, color);
    }

    fn debug_draw_vertical(
        back_buffer: &mut OffscreenBuffer,
        x: i32,
        mut top: i32,
        mut bottom: i32,
        color: u32,
    ) {
        top = top.max(0);
        bottom = bottom.min(back_buffer.height);

        if x >= 0 && x < back_buffer.width {
            let rows = back_buffer
                .memory
                .as_slice_mut()
                .chunks_exact_mut(back_buffer.pitch as usize)
                .skip(top as usize)
                .take((bottom - top) as usize);
            for row in rows {
                if let Some(pixel) = row
                    .chunks_exact_mut(back_buffer.bytes_per_pixel as usize)
                    .nth(x as usize)
                {
                    pixel.copy_from_slice(&color.to_ne_bytes());
                }
            }
        }
    }

    #[derive(Default)]
    pub struct GameCode {
        handle: Option<*mut ()>,
        pub last_write_time: i64,
        update_and_render_stub: Option<handmade::game::UpdateAndRender>,
        get_sound_samples_stub: Option<handmade::game::GetSoundSample>,
    }

    impl GameCode {
        pub fn update_and_render(
            &self,
            thread: &handmade::game::Thread,
            memory: &mut handmade::game::Memory,
            inputs: &mut handmade::game::Input,
            buffer: &mut handmade::game::OffscreenBuffer,
        ) {
            if let Some(func) = self.update_and_render_stub {
                func(thread, memory, inputs, buffer)
            } else {
            }
        }

        pub fn get_sound_samples(
            &self,
            thread: &handmade::game::Thread,
            memory: &mut handmade::game::Memory,
            sound_buffer: &handmade::game::SoundBuffer,
        ) {
            if let Some(func) = self.get_sound_samples_stub {
                func(thread, memory, sound_buffer)
            } else {
            }
        }
    }

    pub fn get_last_write_time(filename: &str) -> Option<i64> {
        const AT_FDCWD: i32 = -100;
        const AT_STATX_SYNC_AS_STAT: i32 = 0x0000;
        const STATX_MTIME: u32 = 0x00000040;

        let mut stat = statx::default();
        if unsafe {
            statx(
                AT_FDCWD,
                filename.as_ptr() as *const _,
                AT_STATX_SYNC_AS_STAT,
                STATX_MTIME,
                std::ptr::addr_of_mut!(stat),
            )
        } == 0
        {
            return Some((stat.stx_mtime.tv_sec << 30) | stat.stx_mtime.tv_nsec as i64);
        }
        None
    }

    pub fn load_game_code(filename: &str) -> GameCode {
        debug_assert!(filename.ends_with('\0'));

        let mut game_code = GameCode::default();
        game_code.last_write_time = get_last_write_time(filename).unwrap();
        unsafe {
            let game_code_library = dlopen(filename.as_ptr() as *const i8, RTLD_NOW);
            game_code.handle = if !game_code_library.is_null() {
                game_code.update_and_render_stub =
                    std::mem::transmute::<*mut std::ffi::c_void, _>(dlsym(
                        game_code_library,
                        "update_and_render\0".as_ptr() as *const i8,
                    ));
                game_code.get_sound_samples_stub =
                    std::mem::transmute::<*mut std::ffi::c_void, _>(dlsym(
                        game_code_library,
                        "get_sound_samples\0".as_ptr() as *const i8,
                    ));
                Some(game_code_library as *mut ())
            } else {
                None
            };
        }
        game_code
    }

    pub fn unload_game_code(game_code: &mut GameCode) {
        if let Some(handle) = game_code.handle {
            unsafe { dlclose(handle as *mut std::ffi::c_void) };
        }

        game_code.update_and_render_stub = None;
        game_code.get_sound_samples_stub = None;
    }

    pub const MFD_CLOEXEC: u32 = 0x0001;

    pub const PROT_READ: i32 = 0x1;
    pub const PROT_WRITE: i32 = 0x2;

    pub const MAP_SHARED: i32 = 0x1;
    pub const MAP_PRIVATE: i32 = 0x2;
    pub const MAP_FAILED: i32 = -1;

    pub const CLOCK_MONOTONIC: i32 = 1;
    pub const CLOCK_MONOTONIC_RAW: i32 = 4;

    pub const TIMER_ABSTIME: i32 = 0x01;

    pub const O_RDONLY: i32 = 0o00;
    pub const O_RDWR: i32 = 0o02;
    pub const O_CREAT: i32 = 0o0100;
    pub const O_TRUNC: i32 = 0o01000;
    pub const O_NONBLOCK: i32 = 0o04000;

    pub const S_IRUSR: i64 = 0o0400;
    pub const S_IWUSR: i64 = 0o0200;
    pub const S_IRGRP: i64 = S_IRUSR >> 3;
    pub const S_IROTH: i64 = S_IRGRP >> 3;

    pub const _SEEK_SET: i32 = 0;
    pub const SEEK_END: i32 = 2;

    pub const RTLD_NOW: i32 = 0x00002;

    #[derive(Default, Clone, Copy)]
    pub struct MemFd {
        pub fd: i32,
        pub addr: *mut u8,
        pub size: usize,
    }

    impl MemFd {
        pub fn is_null(&self) -> bool {
            return self.addr.is_null();
        }
        pub fn as_slice_mut(&mut self) -> &mut [u8] {
            unsafe { std::slice::from_raw_parts_mut(self.addr, self.size) }
        }
    }

    pub fn memfd_alloc(name: &str, size: i64, addr: usize) -> Result<MemFd, std::io::Error> {
        debug_assert!(!name.is_empty() && name.ends_with('\0'), "allocate_memory");

        let fd = unsafe { memfd_create(name.as_ptr() as *const i8, MFD_CLOEXEC) };
        if fd == -1 {
            return Err(std::io::Error::last_os_error());
        }

        let result = unsafe { ftruncate(fd, size) };
        if result == -1 {
            return Err(std::io::Error::last_os_error());
        }

        let addr = unsafe {
            mmap(
                addr as *const std::ffi::c_void,
                size as usize,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            ) as *mut ()
        };
        if addr as i32 == MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }

        Ok(MemFd {
            fd: fd,
            addr: addr as *mut u8,
            size: size as usize,
        })
    }

    pub fn memfd_release(memory: &mut MemFd) {
        unsafe { close(memory.fd) };
        unsafe { munmap(memory.addr as *mut std::ffi::c_void, memory.size as usize) };
        *memory = MemFd::default();
    }

    pub fn cycle_get_count() -> u64 {
        unsafe { core::arch::x86_64::_rdtsc() }
    }

    pub fn get_wall_clock() -> Result<timespec, std::io::Error> {
        let mut timespec = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            if clock_gettime(CLOCK_MONOTONIC_RAW, std::ptr::addr_of_mut!(timespec)) == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(timespec)
            }
        }
    }

    pub fn sleep(sleep_ns: i64) {
        let duration = timespec {
            tv_sec: sleep_ns / 1_000_000_000,
            tv_nsec: sleep_ns % 1_000_000_000,
        };
        debug_assert!(
            unsafe { nanosleep(std::ptr::addr_of!(duration), std::ptr::null_mut(),) } == 0
        );
    }

    pub fn set_timer_slack(slack_ns: i32) -> bool {
        const PR_SET_TIMERSLACK: i32 = 29;
        const PR_GET_TIMERSLACK: i32 = 30;
        unsafe {
            prctl(PR_SET_TIMERSLACK, slack_ns, 0, 0, 0) > 0
                && prctl(PR_GET_TIMERSLACK, 0, 0, 0, 0) == slack_ns
        }
    }

    #[link(name = "c")]
    unsafe extern "C" {
        pub fn memfd_create(
            name: *const std::ffi::c_char,
            oflag: std::ffi::c_uint,
        ) -> std::ffi::c_int;
        pub fn close(fd: std::ffi::c_int) -> std::ffi::c_int;
        pub fn ftruncate(fd: std::ffi::c_int, length: std::ffi::c_long) -> std::ffi::c_int;
        pub fn mmap(
            addr: *const std::ffi::c_void,
            length: usize,
            prot: std::ffi::c_int,
            flags: std::ffi::c_int,
            fd: std::ffi::c_int,
            offset: std::ffi::c_int,
        ) -> *mut std::ffi::c_void;
        pub fn munmap(addr: *mut std::ffi::c_void, len: usize) -> std::ffi::c_int;

        pub fn poll(
            fds: *mut Pollfd,
            nfds: std::ffi::c_ulong,
            timeout: std::ffi::c_int,
        ) -> std::ffi::c_int;

        fn clock_gettime(clockid: std::ffi::c_int, res: *mut timespec) -> std::ffi::c_int;

        pub fn open(path: *const std::ffi::c_char, flag: std::ffi::c_int, ...) -> std::ffi::c_int;
        pub fn lseek(
            fd: std::ffi::c_int,
            offset: std::ffi::c_long,
            whence: std::ffi::c_int,
        ) -> std::ffi::c_long;

        fn nanosleep(requested_time: *const timespec, remaning: *mut timespec) -> std::ffi::c_int;
        fn clock_getres(clockid: std::ffi::c_int, res: *mut timespec) -> std::ffi::c_int;
        pub fn prctl(option: std::ffi::c_int, ...) -> std::ffi::c_int;
    }

    #[repr(C)]
    pub struct Pollfd {
        pub fd: std::ffi::c_int,
        pub events: std::ffi::c_short,
        pub revents: std::ffi::c_short,
    }

    #[derive(Default, Clone)]
    #[repr(C)]
    pub struct timespec {
        pub tv_sec: std::ffi::c_long,
        pub tv_nsec: std::ffi::c_long,
    }

    #[link(name = "dl")]
    unsafe extern "C" {
        pub fn dlclose(handle: *mut std::ffi::c_void) -> std::ffi::c_int;
        pub fn dlopen(
            path: *const std::ffi::c_char,
            flags: std::ffi::c_int,
        ) -> *mut std::ffi::c_void;
        pub fn dlsym(
            handle: *mut std::ffi::c_void,
            symbol: *const std::ffi::c_char,
        ) -> *mut std::ffi::c_void;
    }

    pub fn get_module_path(buf: &mut [u8]) -> &[u8] {
        let bytes_placed = unsafe {
            readlink(
                "/proc/self/exe\0".as_ptr() as *const _,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
            )
        };
        if bytes_placed != -1 {
            &buf[..bytes_placed as usize]
        } else {
            eprintln!("{}", std::io::Error::last_os_error().to_string());
            &buf[..0]
        }
    }

    #[derive(Default, Clone)]
    #[repr(C)]
    struct statx {
        stx_mask: u32,
        stx_blksize: u32,
        stx_attributes: u64,
        stx_nlink: u32,
        stx_uid: u32,
        stx_gid: u32,
        stx_mode: u16,
        __spare0: [u16; 1],
        stx_ino: u64,
        stx_size: u64,
        stx_blocks: u64,
        stx_attributes_mask: u64,
        stx_atime: statx_timestamp,
        stx_btime: statx_timestamp,
        stx_ctime: statx_timestamp,
        stx_mtime: statx_timestamp,
        stx_rdev_major: u32,
        stx_rdev_minor: u32,
        stx_dev_major: u32,
        stx_dev_minor: u32,
        stx_mnt_id: u64,
        stx_dio_mem_align: u32,
        stx_dio_offset_align: u32,
        stx_subvol: u64,
        stx_atomic_write_unit_min: u32,
        stx_atomic_write_unit_max: u32,
        stx_atomic_write_segments_max: u32,
        stx_dio_read_offset_align: u32,
        stx_atomic_write_unit_max_opt: u32,
        __spare2: [u32; 1],
        __spare3: [u64; 8],
    }

    #[derive(Default, Clone)]
    #[repr(C)]
    struct statx_timestamp {
        tv_sec: i64,
        tv_nsec: u32,
        __reserved: i32,
    }

    #[link(name = "c")]
    unsafe extern "C" {
        fn statx(
            dirfd: std::ffi::c_int,
            path: *const std::ffi::c_char,
            flags: std::ffi::c_int,
            mask: std::ffi::c_uint,
            statxbuf: *mut statx,
        ) -> std::ffi::c_int;

        fn readlink(
            path: *const std::ffi::c_char,
            buf: *mut std::ffi::c_char,
            bufsiz: usize,
        ) -> isize;
    }

    #[derive(Clone, Copy)]
    pub struct StateFileName([u8; 256]);
    impl Default for StateFileName {
        fn default() -> Self {
            StateFileName([0; 256])
        }
    }

    #[derive(Default, Clone, Copy)]
    pub struct ReplayBuffer {
        pub filename: StateFileName,
        pub memory_block: MemFd,
    }

    #[derive(Default)]
    pub struct State {
        pub game_memory: MemFd,
        pub replay_buffers: [ReplayBuffer; 4],

        pub recording_fd: i32,
        pub input_recording_index: i32,

        pub play_back_fd: i32,
        pub input_playing_index: i32,

        pub exe_filepath: StateFileName,
        pub last_slash: usize,
    }

    #[link(name = "c")]
    unsafe extern "C" {
        fn read(fd: std::ffi::c_int, buf: *mut std::ffi::c_void, size: usize) -> isize;
        fn write(fd: std::ffi::c_int, buf: *const std::ffi::c_void, size: usize) -> isize;
    }

    pub fn begin_recording_input(state: &mut State, input_recording_index: i32) {
        if let Some(replay_buffer) = state.replay_buffers.get(input_recording_index as usize) {
            state.input_recording_index = input_recording_index;
            let mut filename = StateFileName::default();
            get_input_file_location(state, true, input_recording_index, &mut filename);
            unsafe {
                state.recording_fd = open(
                    filename.0.as_ptr() as *const _,
                    O_RDWR | O_CREAT | O_TRUNC,
                    S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH,
                );
                #[cfg(any())]
                lseek(replay_buffer.memory_block.fd, 0, _SEEK_SET);
                std::ptr::copy_nonoverlapping(
                    state.game_memory.addr,
                    replay_buffer.memory_block.addr,
                    state.game_memory.size,
                );
            }
        }
    }

    pub fn end_recording_input(state: &mut State) {
        unsafe { close(state.recording_fd) };
        state.input_recording_index = 0;
    }

    pub fn begin_input_play_back(state: &mut State, input_playing_index: i32) {
        if let Some(replay_buffer) = state.replay_buffers.get(input_playing_index as usize) {
            state.input_playing_index = input_playing_index;
            let mut filename = StateFileName::default();
            get_input_file_location(state, true, input_playing_index, &mut filename);
            unsafe {
                state.play_back_fd = open(
                    filename.0.as_ptr() as *const _,
                    O_RDONLY,
                    S_IRUSR | S_IRGRP | S_IROTH,
                );
                #[cfg(any())]
                lseek(replay_buffer.memory_block.fd, 0, _SEEK_SET);
                std::ptr::copy_nonoverlapping(
                    replay_buffer.memory_block.addr,
                    state.game_memory.addr,
                    replay_buffer.memory_block.size,
                );
            }
        }
    }

    pub fn end_input_play_back(state: &mut State) {
        unsafe { close(state.play_back_fd) };
        state.input_playing_index = 0;
    }

    pub fn record_input(state: &mut State, mut new_input: handmade::game::Input) {
        unsafe {
            write(
                state.recording_fd,
                std::ptr::addr_of_mut!(new_input) as *mut _,
                std::mem::size_of_val(&new_input),
            );
        }
    }

    pub fn play_back_input(state: &mut State, new_input: &mut handmade::game::Input) {
        unsafe {
            let bytes_read = read(
                state.play_back_fd,
                new_input as *mut handmade::game::Input as *mut _,
                std::mem::size_of_val(new_input),
            );
            if bytes_read == 0 {
                let play_index = state.input_playing_index;
                end_input_play_back(state);
                begin_input_play_back(state, play_index);
                read(
                    state.play_back_fd,
                    new_input as *mut handmade::game::Input as *mut _,
                    std::mem::size_of_val(new_input),
                );
            }
        }
    }

    pub fn get_input_file_location<'a>(
        state: &State,
        input_stream: bool,
        slot_index: i32,
        dest: &'a mut StateFileName,
    ) -> &'a str {
        use std::io::Write;
        let mut buffer = [0u8; 256];
        let mut temp: &mut [u8] = &mut buffer;
        write!(
            temp,
            "loop_edit_{}_{}.hmi\0",
            slot_index,
            if input_stream { "input" } else { "state" }
        )
        .expect("");
        let length = buffer.iter().position(|&c| c == 0).unwrap();
        build_exe_path_filename(state, std::str::from_utf8(&buffer[..length]).unwrap(), dest)
    }

    pub fn get_exe_filename(state: &mut State) {
        let exe_filename = get_module_path(&mut state.exe_filepath.0);
        state.last_slash = exe_filename
            .iter()
            .rev()
            .skip_while(|&c| (*c) as char != '/')
            .count();
    }

    pub fn build_exe_path_filename<'a>(
        state: &State,
        filename: &str,
        dest: &'a mut StateFileName,
    ) -> &'a str {
        crate::cat_strings(
            &state.exe_filepath.0[..state.last_slash],
            filename.as_bytes(),
            &mut dest.0,
        )
        .unwrap()
    }

    pub fn create_mapped_file(filename: &str, size: usize) -> Option<MemFd> {
        unsafe {
            let fd = open(
                filename.as_ptr() as *const _,
                O_RDWR | O_CREAT | O_TRUNC,
                S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH,
            );
            if fd != -1 {
                if ftruncate(fd, size as i64) != -1 {
                    let result = mmap(
                        std::ptr::null_mut(),
                        size,
                        PROT_WRITE | PROT_READ,
                        MAP_SHARED,
                        fd,
                        0,
                    ) as *mut u8;
                    if result as i32 != MAP_FAILED {
                        return Some(MemFd {
                            fd: fd,
                            addr: result,
                            size: size,
                        });
                    }
                }
            }
        }
        None
    }
}

mod libevdev {
    use crate::linux;

    macro_rules! evdev_symbol {
        ($dev:expr, $lib:expr, $field:ident) => {
            $dev.$field = std::mem::transmute::<*mut std::ffi::c_void, _>(linux::dlsym(
                $lib,
                concat!("libevdev_", stringify!($field), "\0").as_ptr() as *const i8,
            ));
        };
    }

    static mut LIBEVDEV: LibEvdev = LibEvdev {
        new: libevdev_new_stub,
        new_from_fd: libevdev_new_from_fd_stub,
        has_event_code: libevdev_has_event_code_stub,
        next_event: libevdev_next_event_stub,
        get_event_value: libevdev_get_event_value_stub,
        get_abs_info: libevdev_get_abs_info_stub,
    };

    const MAX_CONTROLLERS_COUNT: usize = 4;

    pub const EV_KEY: u32 = 0x01;
    pub const EV_ABS: u32 = 0x03;

    pub const ABS_X: u32 = 0x00;
    pub const ABS_Y: u32 = 0x01;
    pub const ABS_HAT0Y: u32 = 0x11;
    pub const ABS_HAT0X: u32 = 0x10;

    pub const BTN_SOUTH: u32 = 0x130;
    pub const BTN_EAST: u32 = 0x131;
    pub const BTN_NORTH: u32 = 0x133;
    pub const BTN_WEST: u32 = 0x134;

    pub const BTN_TL: u32 = 0x136;
    pub const BTN_TR: u32 = 0x137;
    pub const BTN_SELECT: u32 = 0x13a;
    pub const BTN_START: u32 = 0x13b;

    pub fn load_libevdev() {
        unsafe {
            let evdev_library =
                linux::dlopen("libevdev.so\0".as_ptr() as *const i8, linux::RTLD_NOW);
            if !evdev_library.is_null() {
                evdev_symbol!(LIBEVDEV, evdev_library, new);
                evdev_symbol!(LIBEVDEV, evdev_library, new_from_fd);
                evdev_symbol!(LIBEVDEV, evdev_library, has_event_code);
                evdev_symbol!(LIBEVDEV, evdev_library, next_event);
                evdev_symbol!(LIBEVDEV, evdev_library, get_event_value);
                evdev_symbol!(LIBEVDEV, evdev_library, get_abs_info);
            }
        }
    }

    pub fn get_controllers() -> [*mut libevdev; MAX_CONTROLLERS_COUNT] {
        use std::io::Write;
        let mut controllers = [std::ptr::null_mut() as *mut libevdev; MAX_CONTROLLERS_COUNT];
        let mut controller_index = 0;

        let mut buffer = [0u8; 256];
        let mut event_index = 0;
        while controller_index < MAX_CONTROLLERS_COUNT {
            let mut cursor: &mut [u8] = &mut buffer;
            if write!(cursor, "/dev/input/event{}\0", event_index).is_ok() {
                unsafe {
                    let fd = linux::open(
                        buffer.as_ptr() as *const _,
                        linux::O_RDONLY | linux::O_NONBLOCK,
                        linux::S_IRUSR | linux::S_IRGRP,
                    );
                    if (LIBEVDEV.new_from_fd)(fd, controllers.as_mut_ptr().add(controller_index))
                        >= 0
                    {
                        let has_shoulder_left = (LIBEVDEV.has_event_code)(
                            controllers[controller_index],
                            EV_KEY,
                            BTN_TL,
                        ) == 1;
                        let has_start = (LIBEVDEV.has_event_code)(
                            controllers[controller_index],
                            EV_KEY,
                            BTN_START,
                        ) == 1;
                        let has_left_joystick =
                            (LIBEVDEV.has_event_code)(controllers[controller_index], EV_ABS, ABS_X)
                                == 1;
                        if has_shoulder_left && has_start && has_left_joystick {
                            controller_index += 1;
                        } else {
                            linux::close(fd);
                        }
                    } else if std::io::Error::last_os_error().raw_os_error() == Some(2) {
                        break;
                    } else {
                    }
                }
            } else {
                break;
            }
            event_index += 1;
        }

        controllers
    }

    enum ReadFlag {
        Sync = 1,
        Normal = 2,
    }

    enum ReadStatus {
        Success,
        Sync,
        Eagain = -11,
    }

    pub fn get_controller_state(dev: *mut libevdev) -> Result<(), i32> {
        let mut event = input_event::default();
        let mut flag = ReadFlag::Normal as u32;
        unsafe {
            loop {
                let rc = (LIBEVDEV.next_event)(dev, flag, std::ptr::addr_of_mut!(event));
                if rc == ReadStatus::Sync as i32 {
                    flag = ReadFlag::Sync as u32;
                } else if rc == ReadStatus::Eagain as i32 {
                    if flag == ReadFlag::Sync as u32 {
                        flag = ReadFlag::Normal as u32;
                    } else {
                        return Ok(());
                    }
                } else if rc != ReadStatus::Success as i32 {
                    return Err(rc);
                } else {
                }
            }
        }
    }

    pub fn get_controller_value(dev: *mut libevdev, event_type: u32, code: u32) -> i32 {
        unsafe { (LIBEVDEV.get_event_value)(dev, event_type, code) }
    }

    pub fn get_controller_absinfo(dev: *mut libevdev, code: u32) -> u32 {
        unsafe {
            let info = (LIBEVDEV.get_abs_info)(dev, code);
            if info.is_null() {
                0
            } else {
                (*info).flat
            }
        }
    }

    unsafe extern "C" fn libevdev_new_stub() -> *mut libevdev {
        std::ptr::null_mut() as *mut libevdev
    }
    unsafe extern "C" fn libevdev_new_from_fd_stub(
        _fd: std::ffi::c_int,
        _dev: *mut *mut libevdev,
    ) -> std::ffi::c_int {
        -1
    }
    unsafe extern "C" fn libevdev_has_event_code_stub(
        _dev: *const libevdev,
        _type: std::ffi::c_uint,
        _code: std::ffi::c_uint,
    ) -> std::ffi::c_int {
        -1
    }
    unsafe extern "C" fn libevdev_next_event_stub(
        _dev: *const libevdev,
        _type: std::ffi::c_uint,
        _ev: *mut input_event,
    ) -> std::ffi::c_int {
        -1
    }
    unsafe extern "C" fn libevdev_get_event_value_stub(
        _dev: *const libevdev,
        _type: std::ffi::c_uint,
        _code: std::ffi::c_uint,
    ) -> std::ffi::c_int {
        -1
    }
    unsafe extern "C" fn libevdev_get_abs_info_stub(
        _dev: *const libevdev,
        _type: std::ffi::c_uint,
    ) -> *mut input_absinfo {
        std::ptr::null_mut() as *mut input_absinfo
    }

    struct LibEvdev {
        new: unsafe extern "C" fn() -> *mut libevdev,
        new_from_fd: unsafe extern "C" fn(std::ffi::c_int, *mut *mut libevdev) -> std::ffi::c_int,
        has_event_code: unsafe extern "C" fn(
            *const libevdev,
            std::ffi::c_uint,
            std::ffi::c_uint,
        ) -> std::ffi::c_int,
        next_event: unsafe extern "C" fn(
            *const libevdev,
            std::ffi::c_uint,
            *mut input_event,
        ) -> std::ffi::c_int,
        get_event_value: unsafe extern "C" fn(
            *const libevdev,
            std::ffi::c_uint,
            std::ffi::c_uint,
        ) -> std::ffi::c_int,
        get_abs_info: unsafe extern "C" fn(*const libevdev, std::ffi::c_uint) -> *mut input_absinfo,
    }

    #[repr(C)]
    pub struct libevdev {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[derive(Default)]
    #[repr(C)]
    pub struct timeval {
        tv_sec: std::ffi::c_long,
        tv_usec: std::ffi::c_long,
    }
    #[derive(Default)]
    #[repr(C)]
    pub struct input_event {
        time: timeval,
        type_: std::ffi::c_ushort,
        code: std::ffi::c_ushort,
        value: std::ffi::c_int,
    }
    #[derive(Default)]
    #[repr(C)]
    pub struct input_absinfo {
        value: u32,
        minimum: u32,
        maximum: u32,
        fuzz: u32,
        flat: u32,
        resolution: u32,
    }
}

mod wl {
    use crate::{linux, wl, wp_alpha, xdg};

    const WL_DISPLAY_GET_REGISTRY: u32 = 1;

    const WL_COMPOSITOR_CREATE_SURFACE: u32 = 0;

    const WL_REGISTRY_BIND: u32 = 0;

    const WL_SURFACE_ATTACH: u32 = 1;
    const WL_SURFACE_COMMIT: u32 = 6;
    const WL_SURFACE_DAMAGE_BUFFER: u32 = 9;

    const WL_SHM_CREATE_POOL: u32 = 0;

    const WL_SHM_POOL_CREATE_BUFFER: u32 = 0;
    const WL_SHM_POOL_DESTROY: u32 = 1;

    const WL_MARSHAL_FLAG_DESTROY: u32 = 1;

    const WL_SEAT_GET_POINTER: u32 = 0;
    const WL_SEAT_GET_KEYBOARD: u32 = 1;

    const WL_KEYBOARD_RELEASE: u32 = 0;
    const WL_POINTER_RELEASE: u32 = 1;

    pub enum SHMFormat {
        XRGB8888 = 1,
    }

    enum SeatCapability {
        POINTER = 1,
        KEYBOARD = 2,
    }

    enum OutputMode {
        Current = 0x1,
    }

    pub fn display_connect(sock_name: &str) -> Option<*mut wl_display> {
        let name = if sock_name.is_empty() {
            std::ptr::null()
        } else {
            debug_assert!(sock_name.ends_with('\0'));
            sock_name.as_ptr() as *const i8
        };

        let display = unsafe { wl_display_connect(name) };
        if display.is_null() {
            None
        } else {
            Some(display)
        }
    }

    pub fn display_disconnect(display: *mut wl_display) {
        unsafe { wl_display_disconnect(display) }
    }

    pub fn dispatch_pending_events(display: *mut wl_display) -> Result<i32, std::io::Error> {
        const POLLIN: i16 = 0x001;
        const POLLOUT: i16 = 0x004;
        const POLLERR: i16 = 0x008;
        const POLLHUP: i16 = 0x010;

        loop {
            unsafe {
                if wl_display_dispatch_pending(display) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if wl_display_prepare_read(display) != 0 {
                    continue;
                }

                let mut events = POLLIN;
                let fd = wl_display_get_fd(display);

                if wl_display_flush(display) < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.kind() != std::io::ErrorKind::WouldBlock {
                        wl_display_cancel_read(display);
                        return Err(err);
                    }

                    events |= POLLOUT;
                }

                let mut pfd = linux::Pollfd {
                    fd,
                    events,
                    revents: 0,
                };
                loop {
                    if linux::poll(&mut pfd, 1, -1) >= 0 {
                        break;
                    }
                    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    wl_display_cancel_read(display);
                    return Err(std::io::Error::last_os_error());
                }

                if pfd.revents & POLLIN != 0 {
                    if wl_display_read_events(display) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    return Ok(wl_display_dispatch_pending(display));
                }

                if pfd.revents & POLLOUT != 0 {
                    wl_display_cancel_read(display);
                    continue;
                }

                wl_display_cancel_read(display);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Wayland socket poll failed",
                ));
            }
        }
    }

    pub fn display_roundtrip(display: *mut wl_display) -> i32 {
        unsafe { wl_display_roundtrip(display) }
    }

    pub fn display_get_registry(display: *mut wl_display) -> *mut wl_registry {
        let proxy = display as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            wl_proxy_marshal_array_flags(
                proxy,
                WL_DISPLAY_GET_REGISTRY,
                &wl_registry_interface,
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut wl_registry
        }
    }

    pub fn registry_add_listener(
        registry: *mut wl_registry,
        global_state: *mut linux::GlobalState,
    ) {
        unsafe {
            wl_proxy_add_listener(
                registry as *mut wl_proxy,
                std::ptr::addr_of_mut!(registry_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn buffer_add_listener(buffer: *mut wl_buffer, global_state: *mut linux::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                buffer as *mut wl_proxy,
                std::ptr::addr_of_mut!(buffer_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn compositor_create_surface(compositor: *mut wl_compositor) -> *mut wl_surface {
        let proxy = compositor as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            wl_proxy_marshal_array_flags(
                proxy,
                WL_COMPOSITOR_CREATE_SURFACE,
                &wl_surface_interface,
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut wl_surface
        }
    }

    pub fn shm_create_pool(shm: *mut wl_shm, fd: i32, size: i32) -> *mut wl_shm_pool {
        let proxy = shm as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            args[0].n = 0;
            args[1].h = fd;
            args[2].i = size;
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SHM_CREATE_POOL,
                &wl_shm_pool_interface,
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut wl_shm_pool
        }
    }

    pub fn shm_pool_destroy(pool: *mut wl_shm_pool) {
        let proxy = pool as *mut wl_proxy;
        unsafe {
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SHM_POOL_DESTROY,
                std::ptr::null(),
                wl_proxy_get_version(proxy),
                WL_MARSHAL_FLAG_DESTROY,
                std::ptr::null_mut() as *mut wl_argument,
            );
        }
    }

    pub fn surface_attach(surface: *mut wl_surface, buffer: *mut wl_buffer, x: i32, y: i32) {
        let proxy = surface as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            args[0].o = buffer as *mut wl_object;
            args[1].i = x;
            args[2].i = y;
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SURFACE_ATTACH,
                std::ptr::null_mut(),
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }

    pub fn surface_commit(surface: *mut wl_surface) {
        let proxy = surface as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SURFACE_COMMIT,
                std::ptr::null_mut(),
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }

    pub fn surface_damage_buffer(
        surface: *mut wl_surface,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let proxy = surface as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            args[0].i = x;
            args[1].i = y;
            args[2].i = width;
            args[3].i = height;
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SURFACE_DAMAGE_BUFFER,
                std::ptr::null(),
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }

    pub fn shm_pool_create_buffer(
        pool: *mut wl_shm_pool,
        offset: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: u32,
    ) -> *mut wl_buffer {
        let proxy = pool as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            args[0].n = 0;
            args[1].i = offset;
            args[2].i = width;
            args[3].i = height;
            args[4].i = stride;
            args[5].u = format;
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SHM_POOL_CREATE_BUFFER,
                &wl_buffer_interface,
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut wl_buffer
        }
    }

    #[link(name = "wayland-client")]
    unsafe extern "C" {
        static wl_registry_interface: wl_interface;
        static wl_compositor_interface: wl_interface;
        static wl_surface_interface: wl_interface;
        static wl_shm_interface: wl_interface;
        static wl_shm_pool_interface: wl_interface;
        static wl_buffer_interface: wl_interface;
        static wl_seat_interface: wl_interface;
        static wl_pointer_interface: wl_interface;
        static wl_keyboard_interface: wl_interface;
        static wl_output_interface: wl_interface;

        pub fn wl_proxy_get_version(proxy: *mut wl_proxy) -> std::ffi::c_uint;
        pub fn wl_proxy_marshal_array_flags(
            proxy: *mut wl_proxy,
            opcode: std::ffi::c_uint,
            interface: *const wl_interface,
            version: std::ffi::c_uint,
            flags: std::ffi::c_uint,
            args: *mut wl_argument,
        ) -> *mut wl_proxy;
        pub fn wl_proxy_add_listener(
            proxy: *mut wl_proxy,
            implementation: *mut linux::ListenerImplementation,
            data: *mut std::ffi::c_void,
        ) -> std::ffi::c_int;

        fn wl_display_roundtrip(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_connect(name: *const std::ffi::c_char) -> *mut wl_display;
        fn wl_display_disconnect(display: *mut wl_display);
        fn wl_display_dispatch(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_dispatch_pending(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_flush(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_read_events(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_prepare_read(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_cancel_read(display: *mut wl_display);
        fn wl_display_get_fd(display: *mut wl_display) -> std::ffi::c_int;
    }

    #[repr(C)]
    pub struct wl_display {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_compositor {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_surface {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_shm {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_shm_pool {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_registry {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_proxy {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    struct wl_message {
        name: *const std::ffi::c_char,
        signature: *const std::ffi::c_char,
        types: *const *mut wl_interface,
    }
    #[repr(C)]
    pub struct wl_interface {
        name: *const std::ffi::c_char,
        version: std::ffi::c_int,
        method_count: std::ffi::c_int,
        methods: *const wl_message,
        event_count: std::ffi::c_int,
        events: *const wl_message,
    }
    #[repr(C)]
    pub struct wl_object {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_buffer {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_output {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_seat {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_keyboard {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_pointer {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wl_array {
        pub size: usize,
        pub alloc: usize,
        pub data: *mut std::ffi::c_void,
    }
    #[repr(C)]
    pub union wl_argument {
        pub i: std::ffi::c_int,
        pub u: std::ffi::c_uint,
        pub f: std::ffi::c_int,
        pub s: *const std::ffi::c_char,
        pub o: *mut wl_object,
        pub n: std::ffi::c_uint,
        pub a: *mut wl_array,
        pub h: std::ffi::c_int,
    }
    #[repr(C)]
    struct wl_registry_listener {
        global: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_registry,
                std::ffi::c_uint,
                *const std::ffi::c_char,
                std::ffi::c_uint,
            ),
        >,
        global_remove:
            Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_registry, std::ffi::c_uint)>,
    }
    #[repr(C)]
    struct wl_buffer_listener {
        release: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_buffer)>,
    }
    #[repr(C)]
    struct wl_seat_listener {
        capabilities:
            Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_seat, std::ffi::c_uint)>,
        name: Option<
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_seat, *const std::ffi::c_char),
        >,
    }
    #[repr(C)]
    struct wl_keyboard_listener {
        keymap: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_keyboard,
                std::ffi::c_uint,
                std::ffi::c_int,
                std::ffi::c_uint,
            ),
        >,
        enter: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_keyboard,
                std::ffi::c_uint,
                *mut wl_surface,
                *mut wl_array,
            ),
        >,
        leave: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_keyboard,
                std::ffi::c_uint,
                *mut wl_surface,
            ),
        >,
        key: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_keyboard,
                std::ffi::c_uint,
                std::ffi::c_uint,
                std::ffi::c_uint,
                std::ffi::c_uint,
            ),
        >,
        modifiers: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_keyboard,
                std::ffi::c_uint,
                std::ffi::c_uint,
                std::ffi::c_uint,
                std::ffi::c_uint,
                std::ffi::c_uint,
            ),
        >,
        repeat_info: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_keyboard,
                std::ffi::c_int,
                std::ffi::c_int,
            ),
        >,
    }
    type WlFixedT = std::ffi::c_int;
    #[repr(C)]
    struct wl_pointer_listener {
        enter: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                *mut wl_surface,
                WlFixedT,
                WlFixedT,
            ),
        >,
        leave: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                *mut wl_surface,
            ),
        >,
        motion: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                WlFixedT,
                WlFixedT,
            ),
        >,
        button: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                std::ffi::c_uint,
                std::ffi::c_uint,
                std::ffi::c_uint,
            ),
        >,
        axis: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                std::ffi::c_uint,
                WlFixedT,
            ),
        >,
        frame: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_pointer)>,
        axis_source:
            Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_pointer, std::ffi::c_uint)>,
        axis_stop: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                std::ffi::c_uint,
            ),
        >,
        axis_discrete: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                std::ffi::c_int,
            ),
        >,
        axis_value120: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                std::ffi::c_int,
            ),
        >,
        axis_relative_direction: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_pointer,
                std::ffi::c_uint,
                std::ffi::c_uint,
            ),
        >,
        warp: Option<
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_pointer, WlFixedT, WlFixedT),
        >,
    }

    #[repr(C)]
    struct wl_output_listener {
        geometry: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_output,
                std::ffi::c_int,
                std::ffi::c_int,
                std::ffi::c_int,
                std::ffi::c_int,
                std::ffi::c_int,
                *const std::ffi::c_char,
                *const std::ffi::c_char,
                std::ffi::c_int,
            ),
        >,
        mode: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut wl_output,
                std::ffi::c_uint,
                std::ffi::c_int,
                std::ffi::c_int,
                std::ffi::c_int,
            ),
        >,
        done: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_output)>,
        scale: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_output, std::ffi::c_int)>,
        name: Option<
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_output, *const std::ffi::c_char),
        >,
        description: Option<
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_output, *const std::ffi::c_char),
        >,
    }

    #[no_mangle]
    static mut registry_listener: wl_registry_listener = wl_registry_listener {
        global: Some(registry_global),
        global_remove: None,
    };
    #[no_mangle]
    static mut buffer_listener: wl_buffer_listener = wl_buffer_listener {
        release: Some(buffer_release),
    };
    #[no_mangle]
    static mut seat_listener: wl_seat_listener = wl_seat_listener {
        capabilities: Some(seat_capabilities),
        name: Some(seat_name),
    };
    #[no_mangle]
    static mut keyboard_listener: wl_keyboard_listener = wl_keyboard_listener {
        keymap: Some(keyboard_keymap),
        enter: Some(keyboard_enter),
        leave: Some(keyboard_leave),
        key: Some(keyboard_key),
        modifiers: Some(keyboard_modifiers),
        repeat_info: Some(keyboard_repeat_info),
    };
    #[no_mangle]
    static mut pointer_listener: wl_pointer_listener = wl_pointer_listener {
        enter: Some(pointer_enter),
        leave: Some(pointer_leave),
        motion: Some(pointer_motion),
        button: Some(pointer_button),
        axis: Some(pointer_axis),
        frame: Some(pointer_frame),
        axis_source: Some(pointer_axis_source),
        axis_stop: Some(pointer_axis_stop),
        axis_discrete: Some(pointer_axis_discrete),
        axis_value120: Some(pointer_axis_value120),
        axis_relative_direction: Some(pointer_axis_relative_direction),
        warp: Some(pointer_warp),
    };
    #[no_mangle]
    static mut output_listener: wl_output_listener = wl_output_listener {
        geometry: Some(output_geometry),
        mode: Some(output_mode),
        done: Some(output_done),
        scale: Some(output_scale),
        name: Some(output_name),
        description: Some(output_description),
    };

    unsafe extern "C" fn registry_global(
        data: *mut std::ffi::c_void,
        registry: *mut wl_registry,
        name: std::ffi::c_uint,
        interface: *const std::ffi::c_char,
        version: std::ffi::c_uint,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        let interface = std::ffi::CStr::from_ptr(interface);
        if interface == std::ffi::CStr::from_ptr(wl_compositor_interface.name) {
            global_state.compositor =
                registry_bind(registry, name, &wl_compositor_interface, version)
                    as *mut wl_compositor;
        } else if interface == std::ffi::CStr::from_ptr(wl_shm_interface.name) {
            global_state.shm =
                registry_bind(registry, name, &wl_shm_interface, version) as *mut wl_shm;
        } else if interface == std::ffi::CStr::from_ptr(xdg::xdg_wm_base_interface.name) {
            global_state.window_manager =
                registry_bind(registry, name, &xdg::xdg_wm_base_interface, version)
                    as *mut xdg::xdg_wm_base;
            xdg::wm_add_listener(
                global_state.window_manager,
                data.cast::<linux::GlobalState>(),
            );
        } else if interface == std::ffi::CStr::from_ptr(wl_seat_interface.name) {
            global_state.seat =
                registry_bind(registry, name, &wl_seat_interface, version) as *mut wl::wl_seat;
            seat_add_listener(global_state.seat, data.cast::<linux::GlobalState>());
        } else if interface == std::ffi::CStr::from_ptr(wl_output_interface.name) {
            global_state.output =
                registry_bind(registry, name, &wl_output_interface, version) as *mut wl::wl_output;
            output_add_listener(global_state.output, data.cast::<linux::GlobalState>());
        } else if interface
            == std::ffi::CStr::from_ptr(wp_alpha::wp_alpha_modifier_v1_interface.name)
        {
            global_state.alpha = registry_bind(
                registry,
                name,
                &wp_alpha::wp_alpha_modifier_v1_interface,
                version,
            ) as *mut wp_alpha::wp_alpha_modifier_v1;
        }
    }

    pub unsafe extern "C" fn buffer_release(data: *mut std::ffi::c_void, _buffer: *mut wl_buffer) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        global_state
            .buffer_released
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub unsafe extern "C" fn seat_capabilities(
        data: *mut std::ffi::c_void,
        _seat: *mut wl_seat,
        capabilities: std::ffi::c_uint,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();

        let has_keyboard = (capabilities & SeatCapability::KEYBOARD as u32) != 0;
        if has_keyboard && global_state.keyboard.is_null() {
            global_state.keyboard = seat_get_keyboard(global_state.seat);
            keyboard_add_listener(global_state.keyboard, global_state);
        } else if !has_keyboard && !global_state.keyboard.is_null() {
            keyboard_release(global_state.keyboard);
            global_state.keyboard = std::ptr::null_mut();
        }

        let has_pointer = (capabilities & SeatCapability::POINTER as u32) != 0;
        if has_pointer && global_state.pointer.is_null() {
            global_state.pointer = seat_get_pointer(global_state.seat);
            pointer_add_listener(global_state.pointer, global_state);
        } else if !has_pointer && !global_state.pointer.is_null() {
            pointer_release(global_state.pointer);
            global_state.pointer = std::ptr::null_mut();
        }
    }

    pub unsafe extern "C" fn seat_name(
        _data: *mut std::ffi::c_void,
        _seat: *mut wl_seat,
        _capabilities: *const std::ffi::c_char,
    ) {
    }

    pub unsafe extern "C" fn keyboard_keymap(
        _data: *mut std::ffi::c_void,
        _keyboard: *mut wl_keyboard,
        _format: std::ffi::c_uint,
        _fd: std::ffi::c_int,
        _size: std::ffi::c_uint,
    ) {
    }

    unsafe extern "C" fn keyboard_enter(
        _data: *mut std::ffi::c_void,
        _keyboard: *mut wl_keyboard,
        _serial: std::ffi::c_uint,
        _surface: *mut wl_surface,
        _keys: *mut wl_array,
    ) {
    }

    unsafe extern "C" fn keyboard_leave(
        _data: *mut std::ffi::c_void,
        _keyboard: *mut wl_keyboard,
        _serial: std::ffi::c_uint,
        _surface: *mut wl_surface,
    ) {
    }

    unsafe extern "C" fn keyboard_key(
        data: *mut std::ffi::c_void,
        _keyboard: *mut wl_keyboard,
        _serial: std::ffi::c_uint,
        _time: std::ffi::c_uint,
        key: std::ffi::c_uint,
        key_state: std::ffi::c_uint,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        let keyboard_controller = &mut (*global_state.game_input).controllers[0];
        keyboard_controller.is_connected = true;

        let is_down = key_state == 1;
        if key == linux::KeyCode::W as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.move_up(), is_down);
        } else if key == linux::KeyCode::A as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.move_left(), is_down);
        } else if key == linux::KeyCode::S as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.move_down(), is_down);
        } else if key == linux::KeyCode::D as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.move_right(), is_down);
        } else if key == linux::KeyCode::Q as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.left_shoulder(), is_down);
        } else if key == linux::KeyCode::E as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.right_shoulder(), is_down);
        } else if key == linux::KeyCode::UP as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.action_up(), is_down);
        } else if key == linux::KeyCode::LEFT as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.action_left(), is_down);
        } else if key == linux::KeyCode::DOWN as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.action_down(), is_down);
        } else if key == linux::KeyCode::RIGHT as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.action_right(), is_down);
        } else if key == linux::KeyCode::SPACE as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.start(), is_down);
        } else if key == linux::KeyCode::ESC as u32 {
            linux::process_keyboard_message(&mut keyboard_controller.back(), is_down);
        } else if key == linux::KeyCode::P as u32 {
            if is_down {
                global_state.pause = !global_state.pause;
            }
        } else if key == linux::KeyCode::L as u32 {
            let linux_state = &mut (*global_state.linux_state);
            if is_down {
                if linux_state.input_playing_index == 0 {
                    if linux_state.input_recording_index == 0 {
                        linux::begin_recording_input(linux_state, 1);
                    } else {
                        linux::end_recording_input(linux_state);
                        linux::begin_input_play_back(linux_state, 1);
                    }
                } else {
                    linux::end_input_play_back(linux_state);
                }
            }
        } else {
        }
    }

    unsafe extern "C" fn keyboard_modifiers(
        _data: *mut std::ffi::c_void,
        _keyboard: *mut wl_keyboard,
        _serial: std::ffi::c_uint,
        _mods_depressed: std::ffi::c_uint,
        _mods_latched: std::ffi::c_uint,
        _mods_locked: std::ffi::c_uint,
        _group: std::ffi::c_uint,
    ) {
    }

    unsafe extern "C" fn keyboard_repeat_info(
        _data: *mut std::ffi::c_void,
        _keyboard: *mut wl_keyboard,
        _rate: std::ffi::c_int,
        _delay: std::ffi::c_int,
    ) {
    }

    unsafe extern "C" fn pointer_enter(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _serial: std::ffi::c_uint,
        _surface: *mut wl_surface,
        _surface_x: WlFixedT,
        _surface_y: WlFixedT,
    ) {
    }

    unsafe extern "C" fn pointer_leave(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _serial: std::ffi::c_uint,
        _surface: *mut wl_surface,
    ) {
    }

    unsafe extern "C" fn pointer_motion(
        data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _time: std::ffi::c_uint,
        surface_x: WlFixedT,
        surface_y: WlFixedT,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        (*global_state.game_input).mouse_x = (surface_x + 128) >> 8;
        (*global_state.game_input).mouse_y = (surface_y + 128) >> 8;
        (*global_state.game_input).mouse_z = 0;
    }

    unsafe extern "C" fn pointer_button(
        data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _serial: std::ffi::c_uint,
        _time: std::ffi::c_uint,
        button: std::ffi::c_uint,
        state: std::ffi::c_uint,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        let mouse_buttons = &mut (*global_state.game_input).mouse_buttons;

        let is_down = state == 1;
        if button == linux::MouseBtn::LEFT as u32 {
            linux::process_keyboard_message(&mut mouse_buttons[0], is_down);
        } else if button == linux::MouseBtn::MIDDLE as u32 {
            linux::process_keyboard_message(&mut mouse_buttons[1], is_down);
        } else if button == linux::MouseBtn::RIGHT as u32 {
            linux::process_keyboard_message(&mut mouse_buttons[2], is_down);
        } else if button == linux::MouseBtn::EXTRA as u32 {
            linux::process_keyboard_message(&mut mouse_buttons[3], is_down);
        } else if button == linux::MouseBtn::SIDE as u32 {
            linux::process_keyboard_message(&mut mouse_buttons[4], is_down);
        }
    }

    unsafe extern "C" fn pointer_axis(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _time: std::ffi::c_uint,
        _axis: std::ffi::c_uint,
        _value: WlFixedT,
    ) {
    }

    unsafe extern "C" fn pointer_frame(_data: *mut std::ffi::c_void, _pointer: *mut wl_pointer) {}

    unsafe extern "C" fn pointer_axis_source(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _axis_source: std::ffi::c_uint,
    ) {
    }

    unsafe extern "C" fn pointer_axis_stop(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _time: std::ffi::c_uint,
        _axis: std::ffi::c_uint,
    ) {
    }

    unsafe extern "C" fn pointer_axis_discrete(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _axis: std::ffi::c_uint,
        _discrete: std::ffi::c_int,
    ) {
    }

    unsafe extern "C" fn pointer_axis_value120(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _axis: std::ffi::c_uint,
        _value120: std::ffi::c_int,
    ) {
    }

    unsafe extern "C" fn pointer_axis_relative_direction(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _axis: std::ffi::c_uint,
        _direction: std::ffi::c_uint,
    ) {
    }

    unsafe extern "C" fn pointer_warp(
        _data: *mut std::ffi::c_void,
        _pointer: *mut wl_pointer,
        _surface_x: WlFixedT,
        _surface_y: WlFixedT,
    ) {
    }

    unsafe extern "C" fn output_geometry(
        _data: *mut std::ffi::c_void,
        _output: *mut wl_output,
        _x: std::ffi::c_int,
        _y: std::ffi::c_int,
        _physical_width: std::ffi::c_int,
        _physical_height: std::ffi::c_int,
        _subpixel: std::ffi::c_int,
        _make: *const std::ffi::c_char,
        _model: *const std::ffi::c_char,
        _transform: std::ffi::c_int,
    ) {
    }

    unsafe extern "C" fn output_mode(
        data: *mut std::ffi::c_void,
        _output: *mut wl_output,
        flags: std::ffi::c_uint,
        _width: std::ffi::c_int,
        _height: std::ffi::c_int,
        refresh: std::ffi::c_int,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        if flags & OutputMode::Current as u32 != 0 {
            global_state.refresh_rate = refresh / 1e3 as i32;
        }
    }

    unsafe extern "C" fn output_done(_data: *mut std::ffi::c_void, _output: *mut wl_output) {}

    unsafe extern "C" fn output_scale(
        _data: *mut std::ffi::c_void,
        _output: *mut wl_output,
        _factor: std::ffi::c_int,
    ) {
    }

    unsafe extern "C" fn output_name(
        _data: *mut std::ffi::c_void,
        _output: *mut wl_output,
        _name: *const std::ffi::c_char,
    ) {
    }

    unsafe extern "C" fn output_description(
        _data: *mut std::ffi::c_void,
        _output: *mut wl_output,
        _description: *const std::ffi::c_char,
    ) {
    }

    fn registry_bind(
        registry: *mut wl_registry,
        name: u32,
        interface: *const wl_interface,
        version: u32,
    ) -> *mut wl_proxy {
        let proxy = registry as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            args[0].u = name;
            args[1].s = (*interface).name;
            args[2].u = version;
            wl_proxy_marshal_array_flags(
                proxy,
                WL_REGISTRY_BIND,
                interface,
                version,
                0,
                args.as_mut_ptr(),
            )
        }
    }

    fn seat_add_listener(seat: *mut wl_seat, global_state: *mut linux::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                seat as *mut wl_proxy,
                std::ptr::addr_of_mut!(seat_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    fn seat_get_keyboard(seat: *mut wl_seat) -> *mut wl_keyboard {
        let proxy = seat as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            args[0].n = 0;
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SEAT_GET_KEYBOARD,
                &wl_keyboard_interface,
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut wl_keyboard
        }
    }

    fn keyboard_add_listener(keyboard: *mut wl_keyboard, global_state: *mut linux::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                keyboard as *mut wl_proxy,
                std::ptr::addr_of_mut!(keyboard_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    fn keyboard_release(keyboard: *mut wl_keyboard) -> *mut wl_proxy {
        let proxy = keyboard as *mut wl_proxy;
        unsafe {
            wl_proxy_marshal_array_flags(
                proxy,
                WL_KEYBOARD_RELEASE,
                std::ptr::null(),
                wl_proxy_get_version(proxy),
                WL_MARSHAL_FLAG_DESTROY,
                std::ptr::null_mut() as *mut wl_argument,
            )
        }
    }

    fn seat_get_pointer(seat: *mut wl_seat) -> *mut wl_pointer {
        let proxy = seat as *mut wl_proxy;
        unsafe {
            let mut args: [wl_argument; 10] = std::mem::zeroed();
            args[0].n = 0;
            wl_proxy_marshal_array_flags(
                proxy,
                WL_SEAT_GET_POINTER,
                &wl_pointer_interface,
                wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut wl_pointer
        }
    }

    fn pointer_add_listener(pointer: *mut wl_pointer, global_state: *mut linux::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                pointer as *mut wl_proxy,
                std::ptr::addr_of_mut!(pointer_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    fn pointer_release(pointer: *mut wl_pointer) -> *mut wl_proxy {
        let proxy = pointer as *mut wl_proxy;
        unsafe {
            wl_proxy_marshal_array_flags(
                proxy,
                WL_POINTER_RELEASE,
                std::ptr::null(),
                wl_proxy_get_version(proxy),
                WL_MARSHAL_FLAG_DESTROY,
                std::ptr::null_mut() as *mut wl_argument,
            )
        }
    }

    fn output_add_listener(
        output: *mut wl_output,
        global_state: *mut linux::GlobalState,
    ) -> std::ffi::c_int {
        unsafe {
            wl_proxy_add_listener(
                output as *mut wl_proxy,
                std::ptr::addr_of_mut!(output_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            )
        }
    }
}

mod xdg {
    use crate::{linux, wl, wp_alpha};

    const XDG_WM_BASE_GET_XDG_SURFACE: u32 = 2;
    const XDG_WM_BASE_PONG: u32 = 3;

    const XDG_SURFACE_GET_TOPLEVEL: u32 = 1;
    const XDG_SURFACE_ACK_CONFIGURE: u32 = 4;

    const XDG_TOPLEVEL_SET_TITLE: u32 = 2;
    const XDG_TOPLEVEL_SET_MAX_SIZE: u32 = 7;
    const XDG_TOPLEVEL_SET_MIN_SIZE: u32 = 8;

    const XDG_TOPLEVEL_STATE_ACTIVATED: u32 = 4;

    pub fn wm_add_listener(wm: *mut xdg_wm_base, global_state: *mut linux::GlobalState) {
        unsafe {
            wl::wl_proxy_add_listener(
                wm as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(wm_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn surface_add_listener(surface: *mut xdg_surface, global_state: *mut linux::GlobalState) {
        unsafe {
            wl::wl_proxy_add_listener(
                surface as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(surface_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn wm_get_xdg_surface(
        wm: *mut xdg_wm_base,
        surface: *mut wl::wl_surface,
    ) -> *mut xdg_surface {
        let proxy = wm as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            args[0].n = 0;
            args[1].o = surface as *mut wl::wl_object;
            wl::wl_proxy_marshal_array_flags(
                proxy,
                XDG_WM_BASE_GET_XDG_SURFACE,
                &xdg_surface_interface,
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut xdg_surface
        }
    }

    pub fn surface_get_toplevel(surface: *mut xdg_surface) -> *mut xdg_toplevel {
        let proxy = surface as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            wl::wl_proxy_marshal_array_flags(
                proxy,
                XDG_SURFACE_GET_TOPLEVEL,
                &xdg_toplevel_interface,
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut xdg_toplevel
        }
    }

    pub fn toplevel_set_title(toplevel: *mut xdg_toplevel, title: &str) {
        let title_buf = if title.is_empty() {
            std::ptr::null()
        } else {
            debug_assert!(title.ends_with('\0'));
            title.as_ptr() as *const i8
        };

        let proxy = toplevel as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            args[0].s = title_buf;
            wl::wl_proxy_marshal_array_flags(
                proxy,
                XDG_TOPLEVEL_SET_TITLE,
                &xdg_toplevel_interface,
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }

    pub fn toplevel_add_listener(
        toplevel: *mut xdg_toplevel,
        global_state: *mut linux::GlobalState,
    ) {
        unsafe {
            wl::wl_proxy_add_listener(
                toplevel as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(toplevel_listener).cast::<linux::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn toplevel_set_floating(toplevel: *mut xdg_toplevel, width: i32, height: i32) {
        let proxy = toplevel as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            args[0].i = width;
            args[1].i = height;
            wl::wl_proxy_marshal_array_flags(
                proxy,
                XDG_TOPLEVEL_SET_MIN_SIZE,
                std::ptr::null(),
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
            wl::wl_proxy_marshal_array_flags(
                proxy,
                XDG_TOPLEVEL_SET_MAX_SIZE,
                std::ptr::null(),
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }

    #[no_mangle]
    pub static mut wm_listener: xdg_wm_base_listener = xdg_wm_base_listener {
        ping: Some(wm_ping),
    };
    #[no_mangle]
    pub static mut surface_listener: xdg_surface_listener = xdg_surface_listener {
        configure: Some(surface_configure),
    };
    #[no_mangle]
    pub static mut toplevel_listener: xdg_toplevel_listener = xdg_toplevel_listener {
        configure: Some(toplevel_configure),
        close: Some(toplevel_close),
        configure_bounds: Some(toplevel_configure_bounds),
        wm_capabilities: Some(toplevel_wm_capabilities),
    };

    #[link(name = "xdg-shell-protocol", kind = "static")]
    unsafe extern "C" {
        pub static xdg_wm_base_interface: wl::wl_interface;
        pub static xdg_surface_interface: wl::wl_interface;
        pub static xdg_toplevel_interface: wl::wl_interface;
    }
    #[repr(C)]
    pub struct xdg_wm_base {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct xdg_surface {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct xdg_toplevel {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct xdg_wm_base_listener {
        ping:
            Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut xdg_wm_base, std::ffi::c_uint)>,
    }
    #[repr(C)]
    pub struct xdg_surface_listener {
        configure:
            Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut xdg_surface, std::ffi::c_uint)>,
    }
    #[repr(C)]
    pub struct xdg_toplevel_listener {
        configure: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut xdg_toplevel,
                std::ffi::c_int,
                std::ffi::c_int,
                *mut wl::wl_array,
            ),
        >,
        close: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut xdg_toplevel)>,
        configure_bounds: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut xdg_toplevel,
                std::ffi::c_int,
                std::ffi::c_int,
            ),
        >,
        wm_capabilities: Option<
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut xdg_toplevel, *mut wl::wl_array),
        >,
    }

    unsafe extern "C" fn wm_ping(
        _data: *mut std::ffi::c_void,
        xdg_wm_base: *mut xdg_wm_base,
        serial: std::ffi::c_uint,
    ) {
        wm_pong(xdg_wm_base, serial);
    }

    unsafe extern "C" fn surface_configure(
        _data: *mut std::ffi::c_void,
        surface: *mut xdg_surface,
        serial: std::ffi::c_uint,
    ) {
        surface_ack_configure(surface, serial);
    }

    unsafe extern "C" fn toplevel_configure(
        data: *mut std::ffi::c_void,
        _toplevel: *mut xdg_toplevel,
        width: std::ffi::c_int,
        height: std::ffi::c_int,
        states: *mut wl::wl_array,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        linux::resize_shared_buffer(global_state, width, height);

        let arr = &*states;
        let data = arr.data as *const u32;
        let activated = (0..arr.size / std::mem::size_of::<u32>())
            .any(|i| *data.add(i as usize) == XDG_TOPLEVEL_STATE_ACTIVATED);
        if activated {
            wp_alpha::set_multiplier(global_state.alpha_surface, u32::MAX);
        } else {
            wp_alpha::set_multiplier(global_state.alpha_surface, (0.1 * u32::MAX as f32) as u32);
        }
    }

    unsafe extern "C" fn toplevel_close(data: *mut std::ffi::c_void, _toplevel: *mut xdg_toplevel) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        global_state.running = false;
    }

    unsafe extern "C" fn toplevel_configure_bounds(
        _data: *mut std::ffi::c_void,
        _toplevel: *mut xdg_toplevel,
        _width: std::ffi::c_int,
        _height: std::ffi::c_int,
    ) {
    }

    unsafe extern "C" fn toplevel_wm_capabilities(
        _data: *mut std::ffi::c_void,
        _toplevel: *mut xdg_toplevel,
        _capabilities: *mut wl::wl_array,
    ) {
    }

    fn wm_pong(wm: *mut xdg_wm_base, serial: u32) {
        let proxy = wm as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            args[0].u = serial;
            wl::wl_proxy_marshal_array_flags(
                proxy,
                XDG_WM_BASE_PONG,
                std::ptr::null_mut(),
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }

    fn surface_ack_configure(surface: *mut xdg_surface, serial: u32) {
        let proxy = surface as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            args[0].u = serial;
            wl::wl_proxy_marshal_array_flags(
                proxy,
                XDG_SURFACE_ACK_CONFIGURE,
                std::ptr::null_mut(),
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }
}

mod wp_alpha {
    use crate::wl;

    const MODIFIER_V1_GET_SURFACE: u32 = 1;
    const MODIFIER_SURFACE_V1_SET_MULTIPLIER: u32 = 1;

    #[repr(C)]
    pub struct wp_alpha_modifier_v1 {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct wp_alpha_modifier_surface_v1 {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }

    pub fn get_surface(
        manager: *mut wp_alpha_modifier_v1,
        surface: *mut wl::wl_surface,
    ) -> *mut wp_alpha_modifier_surface_v1 {
        let proxy = manager as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            args[0].n = 0;
            args[1].o = surface as *mut wl::wl_object;
            wl::wl_proxy_marshal_array_flags(
                proxy,
                MODIFIER_V1_GET_SURFACE,
                &wp_alpha_modifier_surface_v1_interface,
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            ) as *mut _
        }
    }

    pub fn set_multiplier(surface: *mut wp_alpha_modifier_surface_v1, factor: u32) {
        let proxy = surface as *mut wl::wl_proxy;
        unsafe {
            let mut args: [wl::wl_argument; 10] = std::mem::zeroed();
            args[0].u = factor;
            wl::wl_proxy_marshal_array_flags(
                proxy,
                MODIFIER_SURFACE_V1_SET_MULTIPLIER,
                std::ptr::null(),
                wl::wl_proxy_get_version(proxy),
                0,
                args.as_mut_ptr(),
            );
        }
    }

    #[link(name = "alpha-modifier-protocol", kind = "static")]
    extern "C" {
        pub static wp_alpha_modifier_v1_interface: wl::wl_interface;
        pub static wp_alpha_modifier_surface_v1_interface: wl::wl_interface;
    }
}

mod pw {
    use crate::{handmade, linux, pw};

    const PW_VERSION_STREAM_EVENTS: u32 = 2;

    pub fn init_pipewire_sound(sound_output: &mut linux::SoundOutput) {
        unsafe {
            let mut pod_buffer = [0u8; 1024];
            let mut pod_builder = spa_pod_builder {
                data: pod_buffer.as_mut_ptr() as *mut std::ffi::c_void,
                size: pod_buffer.len() as u32,
                padding: 0,
                state: spa_pod_builder_state {
                    offset: 0,
                    flags: 0,
                    frame: std::ptr::null_mut(),
                },
                callbacks: spa_callbacks {
                    funcs: std::ptr::null(),
                    data: std::ptr::null_mut(),
                },
            };

            pw_init(std::ptr::null_mut(), std::ptr::null_mut());

            sound_output.sound_main_loop = pw_thread_loop_new(
                "handmade-audio\0".as_ptr() as *const i8,
                std::ptr::null_mut(),
            );
            sound_output.sound_loop = pw_thread_loop_get_loop(sound_output.sound_main_loop);
            pw_thread_loop_lock(sound_output.sound_main_loop);

            spa_ringbuffer_init(std::ptr::addr_of_mut!(sound_output.ring));
            const SPA_FD_NONBLOCK: i32 = 1 << 1;
            sound_output.eventfd =
                spa_system_eventfd_create((*sound_output.sound_loop).system, SPA_FD_NONBLOCK);

            let mut property_items = [
                spa_dict_item {
                    key: "media.type\0".as_ptr() as *const i8,
                    value: "Audio\0".as_ptr() as *const i8,
                },
                spa_dict_item {
                    key: "media.category\0".as_ptr() as *const i8,
                    value: "Playback\0".as_ptr() as *const i8,
                },
                spa_dict_item {
                    key: "media.role\0".as_ptr() as *const i8,
                    value: "Music\0".as_ptr() as *const i8,
                },
                spa_dict_item {
                    key: "module.rt\0".as_ptr() as *const i8,
                    value: "false\0".as_ptr() as *const i8,
                },
            ];
            let properties = spa_dict {
                flags: 0,
                n_items: property_items.len() as u32,
                items: property_items.as_mut_ptr(),
            };
            sound_output.stream = pw_stream_new_simple(
                sound_output.sound_loop,
                "handmade-stream\0".as_ptr() as *const i8,
                pw_properties_new_dict(&properties as *const spa_dict),
                std::ptr::addr_of_mut!(stream_events),
                sound_output as *mut linux::SoundOutput as *mut std::ffi::c_void,
            );

            let params = [spa_format_audio_raw_build(
                &mut pod_builder,
                spa_param_type::EnumFormat as u32,
                &spa_audio_info_raw {
                    format: spa_audio_format::S16,
                    flags: 0,
                    channels: sound_output.channels as u32,
                    rate: sound_output.samples_per_second as u32,
                    position: [0; 64],
                },
            )];

            pw_stream_connect(
                sound_output.stream,
                spa_direction::Output,
                0xffffffff,
                StreamFlag::AutoConnect as u32 | StreamFlag::MapBuffers as u32,
                params.as_ptr(),
                params.len() as u32,
            );
            pw_thread_loop_start(sound_output.sound_main_loop);
            pw_thread_loop_unlock(sound_output.sound_main_loop);
        }
    }

    pub fn get_current_position(sound_output: &linux::SoundOutput) -> Option<(u32, u32)> {
        let mut time = pw_time::default();
        if unsafe {
            pw_stream_get_time_n(
                sound_output.stream,
                std::ptr::addr_of_mut!(time),
                std::mem::size_of_val(&time),
            )
        } == 0
        {
            let now = unsafe { pw_stream_get_nsec(sound_output.stream) };
            let diff = (now as i64 - time.now) as f64;
            let elapsed = (time.rate.denom as f64 * diff) / (time.rate.num as f64 * 1e9);
            let play_cursor = time.ticks as f64 + elapsed - time.delay as f64;
            let write_cursor = time.ticks + time.queued;
            Some((
                (play_cursor as i32 * sound_output.bytes_per_sample)
                    .rem_euclid(sound_output.secondary_buffer.size as i32) as u32,
                (write_cursor as i32 * sound_output.bytes_per_sample)
                    .rem_euclid(sound_output.secondary_buffer.size as i32) as u32,
            ))
        } else {
            None
        }
    }

    pub fn ringbuffer_get_write_index(rbuf: &mut spa_ringbuffer, index: &mut u32) -> i32 {
        unsafe { pw::spa_ringbuffer_get_write_index(rbuf as *mut _, index as *mut _) }
    }

    pub fn ringbuffer_write_update(rbuf: &mut spa_ringbuffer, index: i32) {
        unsafe { pw::spa_ringbuffer_write_update(rbuf as *mut _, index) }
    }

    pub fn fill_sound_buffer(
        sound_output: &mut linux::SoundOutput,
        write_index: u32,
        sound_buffer: handmade::game::SoundBuffer,
    ) {
        let secondary_buffer = sound_output.secondary_buffer.as_slice_mut();
        let secondary_buffer_size = secondary_buffer.len();
        let bytes_to_write = sound_buffer.sample_count * sound_output.bytes_per_sample;

        unsafe {
            pw::spa_ringbuffer_write_data(
                std::ptr::addr_of_mut!(sound_output.ring),
                secondary_buffer.as_mut_ptr() as *mut std::ffi::c_void,
                secondary_buffer_size as u32,
                write_index as u32 % secondary_buffer_size as u32,
                sound_buffer.memory as *const std::ffi::c_void,
                bytes_to_write as u32,
            );
            pw::spa_ringbuffer_write_update(
                std::ptr::addr_of_mut!(sound_output.ring),
                write_index as i32 + bytes_to_write as i32,
            )
        };
    }

    #[no_mangle]
    static mut stream_events: pw_stream_events = pw_stream_events {
        version: PW_VERSION_STREAM_EVENTS,
        destroy: None,
        state_changed: None,
        control_info: None,
        io_changed: None,
        param_changed: None,
        add_buffer: None,
        remove_buffer: None,
        process: Some(on_processed),
        drained: None,
        command: None,
        trigger_done: None,
    };

    #[repr(C)]
    pub struct pw_loop {
        pub system: *mut spa_system,
        wrapped_loop: *mut spa_loop,
        control: *mut spa_loop_control,
        utils: *mut spa_loop_utils,
        name: *const std::ffi::c_char,
    }
    #[repr(C)]
    struct pw_stream_events {
        version: u32,
        destroy: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
        state_changed: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                pw_stream_state,
                pw_stream_state,
                *const std::ffi::c_char,
            ),
        >,
        control_info: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut pw_stream_control)>,
        io_changed: Option<
            unsafe extern "C" fn(
                *mut std::ffi::c_void,
                std::ffi::c_uint,
                *mut std::ffi::c_void,
                std::ffi::c_uint,
            ),
        >,
        param_changed:
            Option<unsafe extern "C" fn(*mut std::ffi::c_void, std::ffi::c_uint, *mut spa_pod)>,
        add_buffer: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut pw_buffer)>,
        remove_buffer: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut pw_buffer)>,
        process: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
        drained: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
        command: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut spa_command)>,
        trigger_done: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
    }
    #[repr(C)]
    enum pw_stream_state {
        Error,
        Unconnected,
        Connecting,
        Paused,
        Streaming,
    }
    #[repr(C)]
    struct pw_stream_control {
        flags: std::ffi::c_uint,
        def: std::ffi::c_float,
        min: std::ffi::c_float,
        max: std::ffi::c_float,
        values: *mut std::ffi::c_float,
        n_values: std::ffi::c_uint,
        max_values: std::ffi::c_uint,
    }
    #[repr(C)]
    pub struct pw_buffer {
        pub buffer: *mut spa_buffer,
        pub user_data: *mut std::ffi::c_void,
        pub size: std::ffi::c_ulonglong,
        pub requested: std::ffi::c_ulonglong,
        pub time: std::ffi::c_ulonglong,
    }
    #[repr(C)]
    pub struct pw_properties {
        dict: spa_dict,
        flags: std::ffi::c_uint,
    }

    unsafe extern "C" fn on_processed(data: *mut std::ffi::c_void) {
        let sound_output = &mut *data.cast::<linux::SoundOutput>();

        let stream_buffer = pw::pw_stream_dequeue_buffer(sound_output.stream);
        if stream_buffer.is_null() {
            return;
        }

        let playback_buffer = &mut *stream_buffer;
        let buffers = std::slice::from_raw_parts_mut(
            (*playback_buffer.buffer).datas,
            (*playback_buffer.buffer).n_datas as usize,
        );
        if buffers.is_empty() || buffers[0].data.is_null() {
            pw::pw_stream_return_buffer(sound_output.stream, stream_buffer);
            return;
        }

        let mut read_index = 0;
        let bytes_to_read = spa_ringbuffer_get_read_index(
            std::ptr::addr_of_mut!(sound_output.ring),
            std::ptr::addr_of_mut!(read_index),
        );
        let target_bytes = (playback_buffer.requested as u32
            * sound_output.bytes_per_sample as u32)
            .min(buffers[0].maxsize)
            .min(bytes_to_read.max(0) as u32);
        let secondary_buffer = sound_output.secondary_buffer.as_slice_mut();
        spa_ringbuffer_read_data(
            std::ptr::addr_of_mut!(sound_output.ring),
            secondary_buffer.as_ptr() as *const std::ffi::c_void,
            secondary_buffer.len() as u32,
            read_index % secondary_buffer.len() as u32,
            buffers[0].data,
            target_bytes,
        );
        spa_ringbuffer_read_update(
            std::ptr::addr_of_mut!(sound_output.ring),
            (read_index + target_bytes) as i32,
        );

        let chunk = &mut *(buffers[0].chunk);
        chunk.offset = 0;
        chunk.stride = sound_output.bytes_per_sample;
        chunk.size = target_bytes;
        playback_buffer.size = (target_bytes as i32 / sound_output.bytes_per_sample) as u64;

        pw::pw_stream_queue_buffer(sound_output.stream, playback_buffer);
        pw::spa_system_eventfd_write(
            (*sound_output.sound_loop).system,
            sound_output.eventfd,
            playback_buffer.requested as u64,
        );
    }

    #[link(name = "pipewire-0.3")]
    unsafe extern "C" {
        fn pw_init(argc: *mut std::ffi::c_int, argv: *mut *mut std::ffi::c_char);

        pub fn pw_stream_dequeue_buffer(stream: *mut pw_stream) -> *mut pw_buffer;
        pub fn pw_stream_queue_buffer(
            stream: *mut pw_stream,
            buffer: *mut pw_buffer,
        ) -> std::ffi::c_int;
        pub fn pw_stream_return_buffer(
            stream: *mut pw_stream,
            buffer: *mut pw_buffer,
        ) -> std::ffi::c_int;
        fn pw_stream_new_simple(
            _loop: *mut pw_loop,
            name: *const std::ffi::c_char,
            props: *mut pw_properties,
            events: *const pw_stream_events,
            data: *mut std::ffi::c_void,
        ) -> *mut pw_stream;
        fn pw_stream_connect(
            stream: *mut pw_stream,
            direction: spa_direction,
            target_id: std::ffi::c_uint,
            flags: std::ffi::c_uint,
            params: *const *mut spa_pod,
            n_params: std::ffi::c_uint,
        );
        fn pw_stream_get_time_n(
            stream: *mut pw_stream,
            time: *mut pw_time,
            size: usize,
        ) -> std::ffi::c_int;
        fn pw_stream_get_nsec(stream: *mut pw_stream) -> std::ffi::c_ulonglong;

        fn pw_properties_new_dict(dict: *const spa_dict) -> *mut pw_properties;

        fn pw_thread_loop_new(
            name: *const std::ffi::c_char,
            props: *const spa_dict,
        ) -> *mut pw_thread_loop;
        fn pw_thread_loop_get_loop(audio_loop: *mut pw_thread_loop) -> *mut pw_loop;

        fn pw_thread_loop_start(object: *mut pw_thread_loop) -> std::ffi::c_int;
        fn pw_thread_loop_lock(object: *mut pw_thread_loop);
        fn pw_thread_loop_unlock(object: *mut pw_thread_loop);
    }
    #[derive(Default)]
    #[repr(C)]
    pub struct pw_stream {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct pw_thread_loop {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[derive(Default)]
    #[repr(C)]
    struct pw_time {
        now: std::ffi::c_longlong,
        rate: spa_fraction,
        ticks: std::ffi::c_ulonglong,
        delay: std::ffi::c_longlong,
        queued: std::ffi::c_ulonglong,
        buffered: std::ffi::c_ulonglong,
        queued_buffers: std::ffi::c_uint,
        avail_buffers: std::ffi::c_uint,
        size: std::ffi::c_ulonglong,
    }

    #[link(name = "spa")]
    unsafe extern "C" {
        fn spa_format_audio_raw_build(
            builder: *mut spa_pod_builder,
            id: std::ffi::c_uint,
            info: *const spa_audio_info_raw,
        ) -> *mut spa_pod;

        fn spa_ringbuffer_init(rbuf: *mut spa_ringbuffer);
        fn spa_ringbuffer_get_read_index(
            rbuf: *mut spa_ringbuffer,
            index: *mut std::ffi::c_uint,
        ) -> std::ffi::c_int;
        fn spa_ringbuffer_read_data(
            rbuf: *mut spa_ringbuffer,
            buffer: *const std::ffi::c_void,
            size: std::ffi::c_uint,
            offset: std::ffi::c_uint,
            data: *mut std::ffi::c_void,
            len: std::ffi::c_uint,
        );
        fn spa_ringbuffer_read_update(rbuf: *mut spa_ringbuffer, index: std::ffi::c_int);
        fn spa_ringbuffer_get_write_index(
            rbuf: *mut spa_ringbuffer,
            index: *mut std::ffi::c_uint,
        ) -> std::ffi::c_int;
        fn spa_ringbuffer_write_data(
            rbuf: *mut spa_ringbuffer,
            buffer: *mut std::ffi::c_void,
            size: std::ffi::c_uint,
            offset: std::ffi::c_uint,
            data: *const std::ffi::c_void,
            len: std::ffi::c_uint,
        );
        fn spa_ringbuffer_write_update(rbuf: *mut spa_ringbuffer, index: std::ffi::c_int);

        fn spa_system_eventfd_create(
            object: *mut spa_system,
            flags: std::ffi::c_int,
        ) -> std::ffi::c_int;
        fn spa_system_eventfd_read(
            object: *mut spa_system,
            fd: std::ffi::c_int,
            count: *mut std::ffi::c_ulonglong,
        ) -> std::ffi::c_int;
        fn spa_system_eventfd_write(
            object: *mut spa_system,
            fd: std::ffi::c_int,
            count: std::ffi::c_ulonglong,
        ) -> std::ffi::c_int;
    }
    #[repr(C)]
    struct spa_dict_item {
        key: *const std::ffi::c_char,
        value: *const std::ffi::c_char,
    }
    #[repr(C)]
    struct spa_dict {
        flags: u32,
        n_items: u32,
        items: *mut spa_dict_item,
    }
    #[repr(C)]
    pub struct spa_system {
        iface: spa_interface,
    }
    #[repr(C)]
    struct spa_interface {
        spa_type: *const std::ffi::c_char,
        version: std::ffi::c_uint,
        cb: spa_callbacks,
    }
    #[repr(C)]
    struct spa_callbacks {
        funcs: *const std::ffi::c_void,
        data: *mut std::ffi::c_void,
    }
    #[repr(C)]
    struct spa_loop {
        iface: spa_interface,
    }
    #[repr(C)]
    struct spa_loop_control {
        iface: spa_interface,
    }
    #[repr(C)]
    struct spa_loop_utils {
        iface: spa_interface,
    }
    #[derive(Default)]
    #[repr(C)]
    struct spa_pod {
        size: std::ffi::c_uint,
        id: std::ffi::c_uint,
    }
    #[repr(C)]
    pub struct spa_buffer {
        pub n_metas: std::ffi::c_uint,
        pub n_datas: std::ffi::c_uint,
        pub metas: *mut spa_meta,
        pub datas: *mut spa_data,
    }
    #[repr(C)]
    pub struct spa_meta {
        metadata_type: std::ffi::c_uint,
        size: std::ffi::c_uint,
        data: *mut std::ffi::c_void,
    }
    #[repr(C)]
    pub struct spa_data {
        pub data_type: std::ffi::c_uint,
        pub flags: std::ffi::c_uint,
        pub fd: std::ffi::c_longlong,
        pub offset: std::ffi::c_uint,
        pub maxsize: std::ffi::c_uint,
        pub data: *mut std::ffi::c_void,
        pub chunk: *mut spa_chunk,
    }
    #[repr(C)]
    pub struct spa_chunk {
        pub offset: std::ffi::c_uint,
        pub size: std::ffi::c_uint,
        pub stride: std::ffi::c_int,
        pub flags: std::ffi::c_int,
    }
    #[repr(C)]
    struct spa_command {
        pod: spa_pod,
        body: spa_command_body,
    }
    #[repr(C)]
    struct spa_command_body {
        body: spa_pod_object_body,
    }
    #[repr(C)]
    struct spa_pod_object_body {
        spa_type: std::ffi::c_uint,
        id: std::ffi::c_uint,
    }
    #[repr(C)]
    struct spa_pod_builder {
        data: *mut std::ffi::c_void,
        size: std::ffi::c_uint,
        padding: std::ffi::c_uint,
        state: spa_pod_builder_state,
        callbacks: spa_callbacks,
    }
    #[repr(C)]
    struct spa_pod_builder_state {
        offset: std::ffi::c_uint,
        flags: std::ffi::c_uint,
        frame: *mut spa_pod_frame,
    }
    #[derive(Default)]
    #[repr(C)]
    struct spa_pod_frame {
        pod: spa_pod,
        parent: *mut spa_pod_frame,
        offset: std::ffi::c_uint,
        flags: std::ffi::c_uint,
    }
    #[repr(C)]
    enum spa_param_type {
        EnumFormat = 3,
    }
    #[repr(C)]
    struct spa_audio_info_raw {
        format: spa_audio_format,
        flags: std::ffi::c_uint,
        rate: std::ffi::c_uint,
        channels: std::ffi::c_uint,
        position: [std::ffi::c_uint; 64],
    }
    #[repr(C)]
    enum spa_audio_format {
        S16 = 0x104,
    }
    #[repr(C)]
    enum spa_direction {
        Output = 1,
    }
    enum StreamFlag {
        AutoConnect = 1 << 0,
        MapBuffers = 1 << 2,
    }
    #[derive(Default)]
    #[repr(C)]
    struct spa_fraction {
        num: std::ffi::c_uint,
        denom: std::ffi::c_uint,
    }
    #[derive(Default)]
    #[repr(C)]
    pub struct spa_ringbuffer {
        readindex: std::ffi::c_uint,
        writeindex: std::ffi::c_uint,
    }
}

fn cat_strings<'a>(
    source_a: &[u8],
    source_b: &[u8],
    dest: &'a mut [u8],
) -> Result<&'a str, std::str::Utf8Error> {
    let mut size = source_a.len();
    dest[..size].copy_from_slice(source_a);
    dest[size..size + source_b.len()].copy_from_slice(source_b);
    size += source_b.len();
    dest[size] = 0;
    size += 1;
    std::str::from_utf8(&dest[..size])
}

fn main() {
    linux::set_timer_slack(1_000);

    let mut linux_state = linux::State::default();
    linux::get_exe_filename(&mut linux_state);

    let mut source_gamecode_fullpath = linux::StateFileName::default();
    let source_gamecode_fullpath = linux::build_exe_path_filename(
        &linux_state,
        "libhandmade.so",
        &mut source_gamecode_fullpath,
    );

    libevdev::load_libevdev();

    let mut global_state = linux::GlobalState::default();
    global_state
        .buffer_released
        .store(true, std::sync::atomic::Ordering::Relaxed);
    global_state.running = true;
    global_state.back_buffer.bytes_per_pixel = std::mem::size_of::<i32>() as i32;

    if let Some(display) = wl::display_connect("") {
        let registry = wl::display_get_registry(display);
        wl::registry_add_listener(registry, &mut global_state);
        wl::display_roundtrip(display);

        debug_assert!(!global_state.compositor.is_null());
        global_state.surface = wl::compositor_create_surface(global_state.compositor);
        global_state.window =
            xdg::wm_get_xdg_surface(global_state.window_manager, global_state.surface);
        xdg::surface_add_listener(global_state.window, &mut global_state);
        wl::display_roundtrip(display);

        global_state.toplevel = xdg::surface_get_toplevel(global_state.window);
        xdg::toplevel_set_title(global_state.toplevel, "Handmade Hero\0");
        xdg::toplevel_add_listener(global_state.toplevel, &mut global_state);

        debug_assert!(!global_state.alpha.is_null());
        global_state.alpha_surface =
            wp_alpha::get_surface(global_state.alpha, global_state.surface);

        wl::surface_commit(global_state.surface);
        linux::resize_shared_buffer(&mut global_state, 960, 540);
        xdg::toplevel_set_floating(
            global_state.toplevel,
            global_state.back_buffer.width,
            global_state.back_buffer.height,
        );

        let monitor_refresh_hz = if global_state.refresh_rate > 0 {
            global_state.refresh_rate
        } else {
            60
        };
        let game_update_hz = monitor_refresh_hz as f32 / 2.;
        let target_seconds_per_frame = 1. / game_update_hz as f64;

        let mut sound_output = linux::SoundOutput::default();
        sound_output.samples_per_second = 48000;
        sound_output.channels = 2;
        sound_output.bytes_per_sample = sound_output.channels * std::mem::size_of::<i16>() as i32;
        sound_output.secondary_buffer = linux::memfd_alloc(
            "handmade_sound\0",
            (sound_output.samples_per_second * sound_output.bytes_per_sample) as i64,
            0,
        )
        .unwrap();
        sound_output.safety_bytes = ((sound_output.samples_per_second as f32 / game_update_hz) / 3.)
            as i32
            * sound_output.bytes_per_sample;
        let samples = linux::memfd_alloc(
            "handmade-samples\0",
            sound_output.secondary_buffer.size as i64,
            0,
        )
        .unwrap();
        pw::init_pipewire_sound(&mut sound_output);

        global_state.linux_state = &mut linux_state as *mut _;
        global_state.running = true;

        #[cfg(any())]
        while global_state.running {
            // NOTE: 256 samples latency
            if let Some((play_cursor, write_cursor)) = pw::get_current_position(&sound_output) {
                println!("PC:{} WC:{}", play_cursor, write_cursor);
            }
        }

        let base_address = if cfg!(HANDMADE_INTERNAL) {
            terabytes!(2)
        } else {
            0
        };

        let mut game_memory = handmade::game::Memory::default();
        game_memory.transient_storage_size = megabytes!(64);
        game_memory.permanent_storage_size = gigabytes!(1);
        game_memory.read_entire_file_stub = Some(debug_platform::read_entire_file);
        game_memory.write_entire_file_stub = Some(debug_platform::write_entire_file);
        game_memory.free_file_memory_stub = Some(debug_platform::free_file_memory);

        let allocated_memory = linux::memfd_alloc(
            "\0",
            game_memory.permanent_storage_size as i64 + game_memory.transient_storage_size as i64,
            base_address,
        );

        if let Ok(allocated_memory) = allocated_memory {
            linux_state.game_memory = allocated_memory;
            (
                game_memory.permanent_storage_memory,
                game_memory.transient_storage_memory,
            ) = {
                let (permanent, transient) = linux_state
                    .game_memory
                    .as_slice_mut()
                    .split_at_mut(game_memory.permanent_storage_size);
                (permanent.as_mut_ptr(), transient.as_mut_ptr())
            };

            let mut replay_buffers = [linux::ReplayBuffer::default(); 4];
            for (replay_index, replay) in replay_buffers.iter_mut().enumerate() {
                let filename = linux::get_input_file_location(
                    &linux_state,
                    false,
                    replay_index as i32,
                    &mut replay.filename,
                );
                replay.memory_block =
                    linux::create_mapped_file(filename, linux_state.game_memory.size).unwrap();
            }
            linux_state.replay_buffers = replay_buffers;

            let mut inputs = [handmade::game::Input::default(); 2];
            let controllers = libevdev::get_controllers();
            let max_controller_count = controllers.iter().take_while(|dev| !dev.is_null()).count();
            let max_controller_count = max_controller_count.min(inputs[0].controllers.len() - 1);

            let mut last_wall_clock = linux::get_wall_clock().unwrap();
            let mut flip_wall_clock = linux::get_wall_clock().unwrap();

            let mut debug_time_marker_index = 0;
            let mut debug_time_markers = [linux::DebugTimeMarker::default(); 30];

            let mut audio_latency_bytes;
            let mut audio_latency_seconds;
            let mut sound_is_valid = false;

            let mut game = linux::load_game_code(&source_gamecode_fullpath);

            let mut last_cycle_count = linux::cycle_get_count();
            while global_state.running {
                let new_game_write_time =
                    linux::get_last_write_time(source_gamecode_fullpath).unwrap();
                if new_game_write_time != game.last_write_time {
                    linux::unload_game_code(&mut game);
                    game = linux::load_game_code(&source_gamecode_fullpath);
                }

                let [new_input, old_input] = &mut inputs;
                new_input.dt_for_frame = target_seconds_per_frame as f32;
                global_state.game_input = &mut *new_input as *mut _;

                let old_keyboard_controller = &old_input.controllers[0];
                let new_keyboard_controller = &mut new_input.controllers[0];
                *new_keyboard_controller = handmade::game::ControllerInput::default();
                for (new_button, old_button) in new_keyboard_controller
                    .buttons
                    .iter_mut()
                    .zip(&old_keyboard_controller.buttons)
                {
                    new_button.ended_down = old_button.ended_down;
                }
                new_keyboard_controller.is_connected = old_keyboard_controller.is_connected;
                for (new_button, old_button) in new_input
                    .mouse_buttons
                    .iter_mut()
                    .zip(&old_input.mouse_buttons)
                {
                    new_button.ended_down = old_button.ended_down;
                    new_button.half_transition_count = 0;
                }
                new_input.mouse_x = old_input.mouse_x;
                new_input.mouse_y = old_input.mouse_y;
                new_input.mouse_z = old_input.mouse_z;

                while !global_state
                    .buffer_released
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    wl::dispatch_pending_events(display);
                }

                if !global_state.pause {
                    for controller_index in 0..max_controller_count {
                        let our_controlle_index = controller_index + 1;
                        let old_controller = &mut old_input.controllers[our_controlle_index];
                        let new_controller = &mut new_input.controllers[our_controlle_index];

                        if libevdev::get_controller_state(controllers[controller_index]).is_ok() {
                            new_controller.is_connected = true;
                            new_controller.is_analog = old_controller.is_analog;

                            linux::process_input_digital_button(
                                old_controller.action_up(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_NORTH,
                                ),
                                new_controller.action_up(),
                            );
                            linux::process_input_digital_button(
                                old_controller.action_down(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_SOUTH,
                                ),
                                new_controller.action_down(),
                            );
                            linux::process_input_digital_button(
                                old_controller.action_left(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_WEST,
                                ),
                                new_controller.action_left(),
                            );
                            linux::process_input_digital_button(
                                old_controller.action_right(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_EAST,
                                ),
                                new_controller.action_right(),
                            );
                            linux::process_input_digital_button(
                                old_controller.left_shoulder(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_TL,
                                ),
                                new_controller.left_shoulder(),
                            );
                            linux::process_input_digital_button(
                                old_controller.right_shoulder(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_TR,
                                ),
                                new_controller.right_shoulder(),
                            );
                            linux::process_input_digital_button(
                                old_controller.start(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_NORTH,
                                ),
                                new_controller.start(),
                            );
                            linux::process_input_digital_button(
                                old_controller.back(),
                                libevdev::get_controller_value(
                                    controllers[controller_index],
                                    libevdev::EV_KEY,
                                    libevdev::BTN_SELECT,
                                ),
                                new_controller.back(),
                            );

                            new_controller.stick_average_x = linux::process_input_stick_value(
                                controllers[controller_index],
                                libevdev::ABS_X,
                            );
                            new_controller.stick_average_y = -linux::process_input_stick_value(
                                controllers[controller_index],
                                libevdev::ABS_Y,
                            );
                            if new_controller.stick_average_x != 0.
                                || new_controller.stick_average_y != 0.
                            {
                                new_controller.is_analog = true;
                            }

                            if libevdev::get_controller_value(
                                controllers[controller_index],
                                libevdev::EV_ABS,
                                libevdev::ABS_HAT0Y,
                            ) == -1
                            {
                                new_controller.stick_average_y = 1.;
                                new_controller.is_analog = false;
                            }
                            if libevdev::get_controller_value(
                                controllers[controller_index],
                                libevdev::EV_ABS,
                                libevdev::ABS_HAT0Y,
                            ) == 1
                            {
                                new_controller.stick_average_y = -1.;
                                new_controller.is_analog = false;
                            }
                            if libevdev::get_controller_value(
                                controllers[controller_index],
                                libevdev::EV_ABS,
                                libevdev::ABS_HAT0X,
                            ) == -1
                            {
                                new_controller.stick_average_x = -1.;
                                new_controller.is_analog = false;
                            }
                            if libevdev::get_controller_value(
                                controllers[controller_index],
                                libevdev::EV_ABS,
                                libevdev::ABS_HAT0X,
                            ) == 1
                            {
                                new_controller.stick_average_x = 1.;
                                new_controller.is_analog = false;
                            }

                            let threshold = 0.5;
                            linux::process_input_digital_button(
                                old_controller.move_left(),
                                (new_controller.stick_average_x < -threshold) as i32,
                                new_controller.move_left(),
                            );
                            linux::process_input_digital_button(
                                old_controller.move_right(),
                                (new_controller.stick_average_x > threshold) as i32,
                                new_controller.move_right(),
                            );
                            linux::process_input_digital_button(
                                old_controller.move_up(),
                                (new_controller.stick_average_y > threshold) as i32,
                                new_controller.move_up(),
                            );
                            linux::process_input_digital_button(
                                old_controller.move_down(),
                                (new_controller.stick_average_y < -threshold) as i32,
                                new_controller.move_down(),
                            );
                        } else {
                            new_controller.is_connected = false;
                        }
                    }

                    let thread = handmade::game::Thread::default();

                    let mut buffer = handmade::game::OffscreenBuffer::default();
                    buffer.memory = global_state.back_buffer.memory.addr;
                    buffer.width = global_state.back_buffer.width;
                    buffer.height = global_state.back_buffer.height;
                    buffer.pitch = global_state.back_buffer.pitch;
                    buffer.bytes_per_pixel = global_state.back_buffer.bytes_per_pixel;

                    if linux_state.input_recording_index == 1 {
                        linux::record_input(&mut linux_state, new_input.clone());
                    }
                    if linux_state.input_playing_index == 1 {
                        linux::play_back_input(&mut linux_state, new_input);
                    }

                    game.update_and_render(&thread, &mut game_memory, new_input, &mut buffer);

                    let audio_wall_clock = linux::get_wall_clock().unwrap();
                    let from_begin_to_audio_seconds =
                        linux::get_seconds_elapsed(flip_wall_clock.clone(), audio_wall_clock);

                    if let Some((play_cursor, write_cursor)) =
                        pw::get_current_position(&sound_output)
                    {
                        if !sound_is_valid {
                            sound_is_valid = true;
                            pw::ringbuffer_write_update(
                                &mut sound_output.ring,
                                write_cursor as i32,
                            );
                        }

                        let mut running_byte_index = 0;
                        let filled_bytes = pw::ringbuffer_get_write_index(
                            &mut sound_output.ring,
                            &mut running_byte_index,
                        );
                        debug_assert!(
                            filled_bytes >= 0
                                && filled_bytes < sound_output.secondary_buffer.size as i32
                        );

                        let byte_to_lock =
                            running_byte_index % sound_output.secondary_buffer.size as u32;

                        let expected_sound_bytes_per_frame =
                            (sound_output.samples_per_second as u32 / game_update_hz as u32)
                                * sound_output.bytes_per_sample as u32;
                        let seconds_left_until_flip =
                            target_seconds_per_frame - from_begin_to_audio_seconds;
                        let expected_byte_until_flip = (seconds_left_until_flip
                            / target_seconds_per_frame)
                            * expected_sound_bytes_per_frame as f64;

                        let expected_frame_boundary_byte =
                            play_cursor + expected_byte_until_flip as u32;

                        let mut safe_write_cursor = if write_cursor < play_cursor {
                            play_cursor + sound_output.secondary_buffer.size as u32
                        } else {
                            write_cursor
                        };
                        debug_assert!(safe_write_cursor >= play_cursor);
                        safe_write_cursor += sound_output.safety_bytes as u32;

                        let audio_card_is_low_latency =
                            safe_write_cursor < expected_frame_boundary_byte;

                        let target_cursor = if audio_card_is_low_latency {
                            expected_frame_boundary_byte + expected_sound_bytes_per_frame
                        } else {
                            write_cursor
                                + expected_sound_bytes_per_frame
                                + sound_output.safety_bytes as u32
                        } % sound_output.secondary_buffer.size as u32;

                        let bytes_to_write = if byte_to_lock > target_cursor {
                            sound_output.secondary_buffer.size as u32 - byte_to_lock + target_cursor
                        } else {
                            target_cursor - byte_to_lock
                        };

                        let mut sound_buffer = handmade::game::SoundBuffer::default();
                        sound_buffer.samples_per_second = sound_output.samples_per_second;
                        sound_buffer.sample_count =
                            bytes_to_write as i32 / sound_output.bytes_per_sample;
                        sound_buffer.bytes_per_sample = sound_output.bytes_per_sample;
                        sound_buffer.memory = samples.addr;
                        game.get_sound_samples(&thread, &mut game_memory, &sound_buffer);

                        #[cfg(HANDMADE_INTERNAL)]
                        {
                            let marker = &mut debug_time_markers[debug_time_marker_index];
                            marker.output_play_cursor = play_cursor;
                            marker.output_write_cursor = write_cursor;
                            marker.output_location = byte_to_lock;
                            marker.output_byte_count = bytes_to_write;
                            marker.expected_flip_play_cursor = expected_frame_boundary_byte;

                            let mut unwrapped_write_cursor = write_cursor;
                            if unwrapped_write_cursor < play_cursor {
                                unwrapped_write_cursor += sound_output.secondary_buffer.size as u32;
                            }
                            audio_latency_bytes = unwrapped_write_cursor - play_cursor;
                            audio_latency_seconds = (audio_latency_bytes as f32
                                / sound_output.bytes_per_sample as f32)
                                / sound_output.samples_per_second as f32;

                            if cfg!(any()) {
                                println!(
                                    "BTL:{} TC:{} BTW:{}, PC:{} WC:{} DELTA:{} ({}s)",
                                    byte_to_lock,
                                    target_cursor,
                                    bytes_to_write,
                                    play_cursor,
                                    write_cursor,
                                    audio_latency_bytes,
                                    audio_latency_seconds
                                );
                            }
                        }
                        pw::fill_sound_buffer(&mut sound_output, running_byte_index, sound_buffer);
                    } else {
                        sound_is_valid = false;
                    }

                    let end_wall_clock = linux::get_wall_clock().unwrap();

                    let work_seconds_elapsed =
                        linux::get_seconds_elapsed(last_wall_clock.clone(), end_wall_clock);
                    let mut seconds_elapsed_for_frame = work_seconds_elapsed;
                    if seconds_elapsed_for_frame < target_seconds_per_frame {
                        let sleep_ns = (target_seconds_per_frame - seconds_elapsed_for_frame) * 1e9;
                        let sched_delay = 1_000_000;
                        if sleep_ns > 0. {
                            linux::sleep((sleep_ns).floor() as i64 - sched_delay);
                        }

                        let test_seconds_elapsed_for_frame = linux::get_seconds_elapsed(
                            last_wall_clock.clone(),
                            linux::get_wall_clock().unwrap(),
                        );
                        if test_seconds_elapsed_for_frame < target_seconds_per_frame {}

                        while seconds_elapsed_for_frame < target_seconds_per_frame {
                            seconds_elapsed_for_frame = linux::get_seconds_elapsed(
                                last_wall_clock.clone(),
                                linux::get_wall_clock().unwrap(),
                            );
                        }
                    } else {
                    }

                    let end_cycle_count = linux::cycle_get_count();
                    let end_wall_clock = linux::get_wall_clock().unwrap();

                    #[cfg(HANDMADE_INTERNAL)]
                    #[cfg(any())]
                    linux::debug_sync_display(
                        &mut global_state.back_buffer,
                        &sound_output,
                        (debug_time_marker_index as i64 - 1) as usize,
                        &debug_time_markers,
                        target_seconds_per_frame,
                    );

                    linux::display_buffer_in_window(&mut global_state, 0, 0);

                    flip_wall_clock = linux::get_wall_clock().unwrap();
                    #[cfg(HANDMADE_INTERNAL)]
                    if let Some((play_cursor, write_cursor)) =
                        pw::get_current_position(&sound_output)
                    {
                        let marker = &mut debug_time_markers[debug_time_marker_index];
                        marker.flip_play_cursor = play_cursor;
                        marker.flip_write_cursor = write_cursor;
                    }

                    inputs.swap(0, 1);

                    let cycles_elapsed = end_cycle_count - last_cycle_count;
                    let ms_per_frame =
                        linux::get_seconds_elapsed(last_wall_clock.clone(), end_wall_clock.clone())
                            * 1e3;
                    if cfg!(all()) {
                        let fps = 1e3 / ms_per_frame;
                        let mcpf = cycles_elapsed as f64 / 1e6;
                        println!("{:.02}ms/f, {:.02}f/s, {:.02}mc/f", ms_per_frame, fps, mcpf);
                    }

                    last_cycle_count = end_cycle_count;
                    last_wall_clock = end_wall_clock;

                    #[cfg(HANDMADE_INTERNAL)]
                    {
                        debug_time_marker_index =
                            (debug_time_marker_index + 1) % debug_time_markers.len();
                    }
                }
            }
        }

        wl::display_disconnect(display);
    } else {
        panic!("display_connect.");
    }
}
