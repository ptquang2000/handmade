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

include!("handmade.rs");

#[cfg(HANDMADE_INTERNAL)]
pub mod debug_platform {
    use crate::sys;

    pub fn read_entire_file(filename: &str) -> Option<(*mut (), i64)> {
        assert!(filename.ends_with('\0'));
        unsafe {
            let fd = sys::open(
                filename.as_ptr() as *const std::ffi::c_void,
                sys::O_RDONLY,
                sys::S_IRUSR | sys::S_IRGRP | sys::S_IROTH,
            );
            if fd != -1 {
                let size = sys::lseek(fd, 0, sys::SEEK_END);
                if size != -1 {
                    let result = sys::mmap(
                        std::ptr::null_mut(),
                        size as usize,
                        sys::PROT_READ,
                        sys::MAP_PRIVATE,
                        fd,
                        0,
                    ) as *mut ();
                    if result as i32 != sys::MAP_FAILED {
                        return Some((result, size));
                    }
                } else {
                }
                sys::close(fd);
            } else {
            }
        }
        None
    }

    pub fn write_entire_file(filename: &str, memory: *mut (), memory_size: i64) -> bool {
        assert!(filename.ends_with('\0'));
        unsafe {
            let fd = sys::open(
                filename.as_ptr() as *const std::ffi::c_void,
                sys::O_RDWR | sys::O_CREAT | sys::O_TRUNC,
                sys::S_IRUSR | sys::S_IWUSR | sys::S_IRGRP | sys::S_IROTH,
            );
            if fd != -1 {
                if sys::ftruncate(fd, memory_size) != -1 {
                    let result = sys::mmap(
                        std::ptr::null_mut(),
                        memory_size as usize,
                        sys::PROT_WRITE,
                        sys::MAP_SHARED,
                        fd,
                        0,
                    ) as *mut ();
                    if result as i32 != sys::MAP_FAILED {
                        std::ptr::copy_nonoverlapping(
                            memory as *const u8,
                            result as *mut u8,
                            memory_size as usize,
                        );
                        sys::munmap(result as *mut std::ffi::c_void, memory_size as usize);
                        return true;
                    } else {
                    }
                }
                sys::close(fd);
            } else {
            }
        }
        false
    }

    pub fn free_file_memory(memory: *mut (), size: i64) {
        unsafe { sys::munmap(memory as *mut std::ffi::c_void, size as usize) };
    }
}

mod linux {
    use crate::{game, libevdev, pw, sys, wl, xdg};

    pub enum KeyCode {
        ESC = 1,
        Q = 16,
        W = 17,
        E = 18,
        A = 30,
        S = 31,
        D = 32,
        SPACE = 57,
        UP = 103,
        LEFT = 105,
        RIGHT = 106,
        DOWN = 108,
    }

    #[derive(Default)]
    pub struct GlobalState {
        pub compositor: *mut wl::wl_compositor,
        pub shm: *mut wl::wl_shm,
        pub surface: *mut wl::wl_surface,
        pub buffer: *mut wl::wl_buffer,

        pub seat: *mut wl::wl_seat,
        pub keyboard: *mut wl::wl_keyboard,

        pub window_manager: *mut xdg::xdg_wm_base,
        pub window: *mut xdg::xdg_surface,
        pub toplevel: *mut xdg::xdg_toplevel,

        pub game_input: *mut game::Input,

        pub running: bool,
        pub buffer_released: bool,
        pub back_buffer: OffscreenBuffer,
    }

    #[derive(Default)]
    pub struct SoundOutput {
        pub samples_per_second: u32,
        pub channels: u32,
        pub bytes_per_sample: u32,
        pub tone_hz: u32,
        pub sound_main_loop: *mut pw::pw_main_loop,
        pub sound_loop: *mut pw::pw_loop,
        pub stream: *mut pw::pw_stream,
    }

    pub type ListenerImplementation = unsafe extern "C" fn();

    #[derive(Default)]
    pub struct OffscreenBuffer {
        pub memory: sys::MemFd,
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
        pub bytes_per_pixel: i32,
    }

    pub fn resize_shared_buffer(global_state: &mut GlobalState, width: i32, height: i32) {
        let buffer = &mut global_state.back_buffer;
        buffer.width = width;
        buffer.height = height;
        buffer.pitch = width * buffer.bytes_per_pixel;
        let bitmap_size = height * buffer.pitch;
        if !buffer.memory.is_null() {
            sys::memfd_release(&mut buffer.memory);
        }

        buffer.memory = sys::memfd_alloc("handmade_hero\0", bitmap_size as i64, 0).unwrap();
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
        global_state.buffer_released = false;
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

    pub fn process_keyboard_message(new_state: &mut game::ButtonState, is_down: bool) {
        assert!(new_state.ended_down != is_down);
        new_state.ended_down = is_down;
        new_state.half_transition_count += 1;
    }

    pub fn process_input_digital_button(
        old_state: &game::ButtonState,
        value: i32,
        new_state: &mut game::ButtonState,
    ) {
        new_state.ended_down = value == 1;
        new_state.half_transition_count = (old_state.ended_down != new_state.ended_down) as i32;
    }

    pub fn process_input_stick_value(controller: *mut libevdev::libevdev, code: u32) -> f32 {
        let left_thump_deadzone = libevdev::get_controller_absinfo(controller, code) as i32;
        let value = libevdev::get_controller_value(controller, libevdev::EV_ABS, libevdev::ABS_X);
        if value < -left_thump_deadzone {
            value as f32 / i16::MIN as f32
        } else if value > left_thump_deadzone {
            value as f32 / -i16::MAX as f32
        } else {
            0.
        }
    }
}

mod sys {
    use crate::sys;

    pub const MFD_CLOEXEC: u32 = 0x0001;

    pub const PROT_READ: i32 = 0x1;
    pub const PROT_WRITE: i32 = 0x2;

    pub const MAP_SHARED: i32 = 0x1;
    pub const MAP_PRIVATE: i32 = 0x2;
    pub const MAP_FAILED: i32 = -1;

    pub const CLOCK_MONOTONIC_RAW: i32 = 4;

    pub const O_RDONLY: i32 = 0o00;
    pub const O_RDWR: i32 = 0o02;
    pub const O_CREAT: i32 = 0o0100;
    pub const O_TRUNC: i32 = 0o01000;
    pub const O_NONBLOCK: i32 = 0o04000;

    pub const S_IRUSR: i64 = 0o0400;
    pub const S_IWUSR: i64 = 0o0200;
    pub const S_IRGRP: i64 = S_IRUSR >> 3;
    pub const S_IROTH: i64 = S_IRGRP >> 3;

    pub const SEEK_END: i32 = 2;

    #[derive(Default)]
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

    pub fn memfd_alloc(name: &str, size: i64, addr: usize) -> Option<MemFd> {
        assert!(!name.is_empty() && name.ends_with('\0'), "allocate_memory");

        let fd = unsafe { sys::memfd_create(name.as_ptr() as *const i8, MFD_CLOEXEC) };
        if fd == -1 {
            println!("Failed to memfd_create");
            return None;
        }

        let result = unsafe { sys::ftruncate(fd, size) };
        if result == -1 {
            println!("Failed to ftruncate");
            return None;
        }

        let addr = unsafe {
            sys::mmap(
                addr as *const std::ffi::c_void,
                size as usize,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            ) as *mut ()
        };
        if addr as i32 == MAP_FAILED {
            println!("Failed to mmap");
            return None;
        }

        Some(MemFd {
            fd: fd,
            addr: addr as *mut u8,
            size: size as usize,
        })
    }

    pub fn memfd_release(memory: &mut MemFd) {
        unsafe { sys::close(memory.fd) };
        unsafe { sys::munmap(memory.addr as *mut std::ffi::c_void, memory.size as usize) };
        *memory = MemFd::default();
    }

    pub fn cycle_get_count() -> u64 {
        unsafe { core::arch::x86_64::_rdtsc() }
    }

    pub fn clock_get_time() -> Option<f64> {
        let mut timespec = sys::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            if sys::clock_gettime(sys::CLOCK_MONOTONIC_RAW, std::ptr::addr_of_mut!(timespec)) == -1
            {
                None
            } else {
                Some(timespec.tv_sec as f64 + timespec.tv_nsec as f64 / 1e9)
            }
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

        pub fn open(
            path: *const std::ffi::c_void,
            flag: std::ffi::c_int,
            mode: std::ffi::c_long,
        ) -> std::ffi::c_int;
        pub fn lseek(
            fd: std::ffi::c_int,
            offset: std::ffi::c_long,
            whence: std::ffi::c_int,
        ) -> std::ffi::c_long;
    }

    #[repr(C)]
    pub struct Pollfd {
        pub fd: std::ffi::c_int,
        pub events: std::ffi::c_short,
        pub revents: std::ffi::c_short,
    }
    #[repr(C)]
    struct timespec {
        tv_sec: std::ffi::c_long,
        tv_nsec: std::ffi::c_long,
    }

    #[link(name = "dl")]
    unsafe extern "C" {
        pub fn dlopen(
            path: *const std::ffi::c_char,
            flags: std::ffi::c_int,
        ) -> *mut std::ffi::c_void;
        pub fn dlsym(
            handle: *mut std::ffi::c_void,
            symbol: *const std::ffi::c_char,
        ) -> *mut std::ffi::c_void;
    }
}

mod libevdev {
    use crate::sys;

    macro_rules! evdev_symbol {
        ($dev:expr, $lib:expr, $field:ident) => {
            $dev.$field = std::mem::transmute::<*mut std::ffi::c_void, _>(sys::dlsym(
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
        const RTLD_NOW: i32 = 0x00002;
        unsafe {
            let evdev_library = sys::dlopen("libevdev.so\0".as_ptr() as *const i8, RTLD_NOW);
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
                    let fd = sys::open(
                        buffer.as_ptr() as *const std::ffi::c_void,
                        sys::O_RDONLY | sys::O_NONBLOCK,
                        sys::S_IRUSR | sys::S_IRGRP,
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
                            sys::close(fd);
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
        Normal,
    }

    enum ReadStatus {
        Success,
        Sync,
        Eagain = -11,
    }

    pub fn get_controller_state(dev: *mut libevdev) -> Result<(), i32> {
        let mut event = input_event::default();
        unsafe {
            let mut rc =
                (LIBEVDEV.next_event)(dev, ReadFlag::Normal as u32, std::ptr::addr_of_mut!(event));
            while rc == ReadStatus::Sync as i32 {
                rc = (LIBEVDEV.next_event)(
                    dev,
                    ReadFlag::Sync as u32,
                    std::ptr::addr_of_mut!(event),
                );
            }
            if rc == ReadStatus::Eagain as i32 || rc == ReadStatus::Success as i32 {
                Ok(())
            } else {
                Err(rc)
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
    use crate::{linux, sys, wl, xdg};

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

    const WL_SEAT_GET_KEYBOARD: u32 = 1;

    const WL_KEYBOARD_RELEASE: u32 = 0;

    pub enum SHMFormat {
        XRGB8888 = 1,
    }

    enum SeatCapability {
        KEYBOARD = 2,
    }

    pub fn display_connect(sock_name: &str) -> Option<*mut wl_display> {
        let name = if sock_name.is_empty() {
            std::ptr::null()
        } else {
            assert!(sock_name.ends_with('\0'));
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

    pub fn dispatch_pending_events(display: *mut wl_display) -> i32 {
        unsafe {
            while wl_display_prepare_read(display) != 0 {
                wl_display_dispatch_pending(display);
            }
            wl_display_flush(display);

            const POLLIN: i16 = 0x001;
            let mut fds = sys::Pollfd {
                fd: wl_display_get_fd(display),
                events: POLLIN,
                revents: 0,
            };
            let nfds = 1;
            if sys::poll(&mut fds, nfds, 0) == -1 || fds.revents == 0 {
                wl_display_cancel_read(display);
            } else {
                assert!(fds.revents == POLLIN, "{}", fds.revents);
                wl_display_read_events(display);
            }

            wl_display_dispatch_pending(display)
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
        static wl_keyboard_interface: wl_interface;

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
        fn wl_display_dispatch_pending(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_flush(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_read_events(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_prepare_read(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_cancel_read(display: *mut wl_display) -> std::ffi::c_int;
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
    pub struct wl_array {
        size: usize,
        alloc: usize,
        data: *mut std::ffi::c_void,
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
        }
    }

    pub unsafe extern "C" fn buffer_release(data: *mut std::ffi::c_void, _buffer: *mut wl_buffer) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        global_state.buffer_released = true;
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
}

mod xdg {
    use crate::{linux, wl};

    const XDG_WM_BASE_GET_XDG_SURFACE: u32 = 2;
    const XDG_WM_BASE_PONG: u32 = 3;

    const XDG_SURFACE_GET_TOPLEVEL: u32 = 1;
    const XDG_SURFACE_ACK_CONFIGURE: u32 = 4;

    const XDG_TOPLEVEL_SET_TITLE: u32 = 2;

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
            assert!(title.ends_with('\0'));
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
        _states: *mut wl::wl_array,
    ) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        linux::resize_shared_buffer(global_state, width, height);
    }

    unsafe extern "C" fn toplevel_close(data: *mut std::ffi::c_void, _toplevel: *mut xdg_toplevel) {
        let global_state = &mut *data.cast::<linux::GlobalState>();
        global_state.running = false;
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

mod pw {
    use crate::{game, linux, pw};

    const PW_VERSION_STREAM_EVENTS: u32 = 2;

    pub fn init(sound_output: &mut linux::SoundOutput) {
        unsafe {
            pw_init(std::ptr::null_mut(), std::ptr::null_mut());

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
            sound_output.sound_main_loop = pw_main_loop_new(std::ptr::null_mut());
            sound_output.sound_loop = pw_main_loop_get_loop(sound_output.sound_main_loop);

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
                    format: spa_audio_format::F32,
                    flags: 0,
                    channels: sound_output.channels,
                    rate: sound_output.samples_per_second,
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
        }
    }

    pub fn loop_iterate(sound_loop: *mut pw_loop) {
        unsafe { pw::pw_loop_iterate(sound_loop, 0) };
    }

    pub fn loop_enter(sound_loop: *mut pw_loop) {
        unsafe { pw::pw_loop_enter(sound_loop) };
    }

    pub fn loop_leave(sound_loop: *mut pw_loop) {
        unsafe { pw::pw_loop_leave(sound_loop) };
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
        system: *mut spa_system,
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
    #[allow(dead_code)]
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
    struct pw_buffer {
        buffer: *mut spa_buffer,
        user_data: *mut std::ffi::c_void,
        size: std::ffi::c_longlong,
        requested: std::ffi::c_longlong,
        time: std::ffi::c_longlong,
    }
    #[repr(C)]
    pub struct pw_properties {
        dict: spa_dict,
        flags: std::ffi::c_uint,
    }

    unsafe extern "C" fn on_processed(userdata: *mut std::ffi::c_void) {
        let sound_output = &mut *userdata.cast::<linux::SoundOutput>();

        let playback_buffer = pw_stream_dequeue_buffer(sound_output.stream);
        if playback_buffer.is_null() {
            println!("out of buffers");
            return;
        }
        let playback_buffer = &mut *playback_buffer;

        let buffers = std::slice::from_raw_parts_mut(
            (*playback_buffer.buffer).datas,
            (*playback_buffer.buffer).n_datas as usize,
        );
        if buffers.is_empty() || buffers[0].data.is_null() {
            return;
        }

        let buffer_size = (playback_buffer.requested.max(1) as u32 * sound_output.bytes_per_sample)
            .min(buffers[0].maxsize);

        game::output_sound(
            game::SoundOutputBuffer {
                samples: std::slice::from_raw_parts_mut(
                    buffers[0].data as *mut u8,
                    buffer_size as usize,
                ),
                samples_per_second: sound_output.samples_per_second,
                bytes_per_sample: sound_output.bytes_per_sample,
            },
            sound_output.tone_hz,
        );

        let chunk = &mut *(buffers[0].chunk);
        chunk.offset = 0;
        chunk.stride = sound_output.bytes_per_sample as i32;
        chunk.size = buffer_size;

        pw_stream_queue_buffer(sound_output.stream, playback_buffer);
    }

    #[link(name = "pipewire-0.3")]
    unsafe extern "C" {
        fn pw_init(argc: *mut std::ffi::c_int, argv: *mut *mut std::ffi::c_char);

        fn pw_stream_dequeue_buffer(stream: *mut pw_stream) -> *mut pw_buffer;
        fn pw_stream_queue_buffer(
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

        fn pw_properties_new_dict(dict: *const spa_dict) -> *mut pw_properties;

        fn pw_main_loop_new(props: *const spa_dict) -> *mut pw_main_loop;
        fn pw_main_loop_get_loop(audio_loop: *mut pw_main_loop) -> *mut pw_loop;

        fn pw_loop_enter(object: *mut pw_loop);
        fn pw_loop_leave(object: *mut pw_loop);
        fn pw_loop_iterate(object: *mut pw_loop, timeout: std::ffi::c_int) -> std::ffi::c_int;
    }
    #[derive(Default)]
    #[repr(C)]
    pub struct pw_stream {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct pw_main_loop {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }

    #[link(name = "spa")]
    unsafe extern "C" {
        fn spa_format_audio_raw_build(
            builder: *mut spa_pod_builder,
            id: std::ffi::c_uint,
            info: *const spa_audio_info_raw,
        ) -> *mut spa_pod;
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
    struct spa_buffer {
        n_metas: std::ffi::c_uint,
        n_datas: std::ffi::c_uint,
        metas: *mut spa_meta,
        datas: *mut spa_data,
    }
    #[repr(C)]
    struct spa_meta {
        metadata_type: std::ffi::c_uint,
        size: std::ffi::c_uint,
        data: *mut std::ffi::c_void,
    }
    #[repr(C)]
    struct spa_data {
        data_type: std::ffi::c_uint,
        flags: std::ffi::c_uint,
        fd: std::ffi::c_longlong,
        offset: std::ffi::c_uint,
        maxsize: std::ffi::c_uint,
        data: *mut std::ffi::c_void,
        chunk: *mut spa_chunk,
    }
    #[repr(C)]
    struct spa_chunk {
        offset: std::ffi::c_uint,
        size: std::ffi::c_uint,
        stride: std::ffi::c_int,
        flags: std::ffi::c_int,
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
        F32 = 0x11B,
    }
    #[repr(C)]
    enum spa_direction {
        Output = 1,
    }
    enum StreamFlag {
        AutoConnect = 1 << 0,
        MapBuffers = 1 << 2,
    }
}

fn main() {
    libevdev::load_libevdev();

    let mut global_state: linux::GlobalState = linux::GlobalState::default();
    global_state.buffer_released = true;
    global_state.running = true;
    global_state.back_buffer.bytes_per_pixel = std::mem::size_of::<i32>() as i32;

    if let Some(display) = wl::display_connect("") {
        let registry = wl::display_get_registry(display);
        wl::registry_add_listener(registry, &mut global_state);
        wl::display_roundtrip(display);

        assert!(!global_state.compositor.is_null());
        global_state.surface = wl::compositor_create_surface(global_state.compositor);
        global_state.window =
            xdg::wm_get_xdg_surface(global_state.window_manager, global_state.surface);
        xdg::surface_add_listener(global_state.window, &mut global_state);

        global_state.toplevel = xdg::surface_get_toplevel(global_state.window);
        xdg::toplevel_set_title(global_state.toplevel, "Handmade Hero\0");
        xdg::toplevel_add_listener(global_state.toplevel, &mut global_state);

        wl::surface_commit(global_state.surface);
        linux::resize_shared_buffer(&mut global_state, 1280, 720);

        let mut sound_output = linux::SoundOutput {
            samples_per_second: 48000,
            bytes_per_sample: 0,
            channels: 2,
            tone_hz: 256,
            sound_main_loop: std::ptr::null_mut(),
            sound_loop: std::ptr::null_mut(),
            stream: std::ptr::null_mut(),
        };
        sound_output.bytes_per_sample = sound_output.channels * std::mem::size_of::<f32>() as u32;
        pw::init(&mut sound_output);
        pw::loop_enter(sound_output.sound_loop);

        let mut inputs = [game::Input::default(); 2];
        let controllers = libevdev::get_controllers();
        let max_controller_count = controllers.iter().take_while(|dev| !dev.is_null()).count();
        let max_controller_count =
            std::cmp::min(max_controller_count, inputs[0].controllers.len() - 1);

        let permanent_storage_size = megabytes!(64);
        let transient_storage_size = gigabytes!(4);
        let base_address = if cfg!(HANDMADE_INTERNAL) {
            terabytes!(2)
        } else {
            0
        };
        let allocated_memory = sys::memfd_alloc(
            "\0",
            permanent_storage_size + transient_storage_size,
            base_address,
        );
        if let Some(mut allocated_memory) = allocated_memory {
            let (permanent_storage, transient_storage) = allocated_memory
                .as_slice_mut()
                .split_at_mut(permanent_storage_size as usize);
            let mut game_memory = game::Memory {
                is_initialized: false,
                permanent_storage: permanent_storage,
                transient_storage: transient_storage,
            };

            let mut last_timestamp = sys::clock_get_time().unwrap();
            let mut last_cycle_count = sys::cycle_get_count();

            while global_state.running {
                let [new_input, old_input] = &mut inputs;
                global_state.game_input = &mut *new_input as *mut _;

                let old_keyboard_controller = &old_input.controllers[0];
                let new_keyboard_controller = &mut new_input.controllers[0];
                *new_keyboard_controller = game::ControllerInput::default();
                for (new_button, old_button) in new_keyboard_controller
                    .buttons
                    .iter_mut()
                    .zip(&old_keyboard_controller.buttons)
                {
                    new_button.ended_down = old_button.ended_down;
                }
                new_keyboard_controller.is_connected = old_keyboard_controller.is_connected;

                loop {
                    let dispatched_events = wl::dispatch_pending_events(display);
                    if dispatched_events == -1 {
                        global_state.running = false;
                        break;
                    } else if global_state.buffer_released && dispatched_events == 0 {
                        break;
                    }
                }

                for controller_index in 0..max_controller_count {
                    let our_controlle_index = controller_index + 1;
                    let old_controller = &mut old_input.controllers[our_controlle_index];
                    let new_controller = &mut new_input.controllers[our_controlle_index];

                    if libevdev::get_controller_state(controllers[controller_index]).is_ok() {
                        new_controller.is_connected = true;
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

                        new_controller.is_analog = true;
                        new_controller.stick_average_x = linux::process_input_stick_value(
                            controllers[controller_index],
                            libevdev::ABS_X,
                        );
                        new_controller.stick_average_y = linux::process_input_stick_value(
                            controllers[controller_index],
                            libevdev::ABS_Y,
                        );
                        if libevdev::get_controller_value(
                            controllers[controller_index],
                            libevdev::EV_ABS,
                            libevdev::ABS_HAT0Y,
                        ) == -1
                        {
                            new_controller.stick_average_y = 1.;
                        }
                        if libevdev::get_controller_value(
                            controllers[controller_index],
                            libevdev::EV_ABS,
                            libevdev::ABS_HAT0Y,
                        ) == 1
                        {
                            new_controller.stick_average_y = -1.;
                        }
                        if libevdev::get_controller_value(
                            controllers[controller_index],
                            libevdev::EV_ABS,
                            libevdev::ABS_HAT0X,
                        ) == -1
                        {
                            new_controller.stick_average_x = -1.;
                        }
                        if libevdev::get_controller_value(
                            controllers[controller_index],
                            libevdev::EV_ABS,
                            libevdev::ABS_HAT0X,
                        ) == 1
                        {
                            new_controller.stick_average_x = 1.;
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
                            (new_controller.stick_average_y < -threshold) as i32,
                            new_controller.move_up(),
                        );
                        linux::process_input_digital_button(
                            old_controller.move_down(),
                            (new_controller.stick_average_y > threshold) as i32,
                            new_controller.move_down(),
                        );
                    } else {
                        new_controller.is_connected = false;
                    }
                }

                pw::loop_iterate(sound_output.sound_loop);

                game::update_and_render(
                    &mut game_memory,
                    new_input.clone(),
                    game::OffscreenBuffer {
                        memory: global_state.back_buffer.memory.as_slice_mut(),
                        width: global_state.back_buffer.width,
                        height: global_state.back_buffer.height,
                        pitch: global_state.back_buffer.pitch,
                        bytes_per_pixel: global_state.back_buffer.bytes_per_pixel,
                    },
                );
                linux::display_buffer_in_window(&mut global_state, 0, 0);

                let end_cycle_count = sys::cycle_get_count();
                let end_timestamp = sys::clock_get_time().unwrap();

                let cycles_elapsed = end_cycle_count - last_cycle_count;
                let ms_per_frame = (end_timestamp - last_timestamp) * 1e3;
                let fps = 1e3 / ms_per_frame;
                let mcpf = cycles_elapsed as f64 / 1e6;
                println!("{:.02}ms/f, {:.02}f/s, {:.02}mc/f", ms_per_frame, fps, mcpf);

                last_cycle_count = end_cycle_count;
                last_timestamp = end_timestamp;

                inputs.swap(0, 1);
            }
        }

        pw::loop_leave(sound_output.sound_loop);
        wl::display_disconnect(display);
    } else {
        panic!("display_connect.");
    }
}
