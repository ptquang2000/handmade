fn render_weird_gradient(x_offset: i32, y_offset: i32, buffer: &mut unix::OffscreenBuffer) {
    let rows = buffer
        .memory
        .as_slice_mut()
        .chunks_exact_mut(buffer.pitch as usize);
    for (y, row) in rows.enumerate() {
        let pixels = row.chunks_exact_mut(buffer.bytes_per_pixel as usize);
        for (x, pixel) in pixels.enumerate() {
            let blue = (x as i32 + x_offset) & 0xFF;
            let green = (y as i32 + y_offset) & 0xFF;
            pixel.copy_from_slice(&(blue | green << 8).to_ne_bytes());
        }
    }
}

mod unix {
    use *;

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

    pub enum EventType {
        None,
        Close,
        Keyboard(u32, u32),
    }

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

        pub event: unix::EventType,
        pub buffer_released: bool,
        pub back_buffer: unix::OffscreenBuffer,
    }

    pub struct SoundOutput {
        pub samples_per_second: u32,
        pub channels: u32,
        pub bytes_per_sample: u32,
        pub tone_hz: u32,
        pub tone_volume: f32,
        pub wave_period: f32,
        pub sound_main_loop: *mut pw::pw_main_loop,
        pub sound_loop: *mut pw::pw_loop,
        pub stream: *mut pw::pw_stream,
        pub t_sine: f32,
    }

    impl Default for GlobalState {
        fn default() -> Self {
            GlobalState {
                compositor: std::ptr::null_mut(),
                shm: std::ptr::null_mut(),
                surface: std::ptr::null_mut(),
                buffer: std::ptr::null_mut(),

                seat: std::ptr::null_mut(),
                keyboard: std::ptr::null_mut(),

                window_manager: std::ptr::null_mut(),
                window: std::ptr::null_mut(),
                toplevel: std::ptr::null_mut(),

                event: unix::EventType::None,
                buffer_released: true,
                back_buffer: unix::OffscreenBuffer::default(),
            }
        }
    }

    pub type ListenerImplementation = unsafe extern "C" fn();

    #[derive(Default)]
    pub struct OffscreenBuffer {
        pub memory: posix::MemFd,
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
        pub bytes_per_pixel: i32,
    }

    pub fn resize_shared_buffer(global_state: &mut unix::GlobalState, width: i32, height: i32) {
        let buffer = &mut global_state.back_buffer;
        buffer.width = width;
        buffer.height = height;
        buffer.pitch = width * buffer.bytes_per_pixel;
        let bitmap_size = height * buffer.pitch;
        if !buffer.memory.is_null() {
            posix::memfd_release(&mut buffer.memory);
        }

        buffer.memory = posix::memfd_alloc("handmade_hero\0", bitmap_size);
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

    pub fn display_buffer_in_window(global_state: &mut unix::GlobalState, x: i32, y: i32) {
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
}

mod posix {
    use *;

    #[derive(Default)]
    pub struct MemFd {
        pub fd: i32,
        addr: *mut u8,
        size: usize,
    }

    impl MemFd {
        pub fn is_null(&self) -> bool {
            return self.addr.is_null();
        }
        pub fn as_slice_mut(&mut self) -> &mut [u8] {
            unsafe { std::slice::from_raw_parts_mut(self.addr, self.size) }
        }
    }

    pub fn memfd_alloc(name: &str, size: i32) -> MemFd {
        const MFD_CLOEXEC: u32 = 0x0001;
        const PROT_READ: i32 = 0x1;
        const PROT_WRITE: i32 = 0x2;
        const MAP_SHARED: i32 = 0x1;
        const MAP_FAILED: i32 = -1;

        assert!(!name.is_empty() && name.ends_with('\0'), "allocate_memory");

        let fd = unsafe { posix::memfd_create(name.as_ptr() as *const i8, MFD_CLOEXEC) };
        assert!(fd != -1, "memfd_create");
        let result = unsafe { posix::ftruncate(fd, size) };
        assert!(result != -1, "ftruncate");
        let addr = unsafe {
            posix::mmap(
                std::ptr::null_mut(),
                size as usize,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            ) as *mut ()
        };
        assert!(addr as i32 != MAP_FAILED, "mmap");
        MemFd {
            fd: fd,
            addr: addr as *mut u8,
            size: size as usize,
        }
    }

    pub fn memfd_release(memory: &mut MemFd) {
        unsafe { posix::close(memory.fd) };
        unsafe { posix::munmap(memory.addr as *mut std::ffi::c_void, memory.size as usize) };
        *memory = MemFd::default();
    }

    pub fn cycle_get_count() -> u64 {
        unsafe { core::arch::x86_64::_rdtsc() }
    }

    pub fn clock_get_time() -> Option<f64> {
        let mut timespec = posix::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            if posix::clock_gettime(posix::CLOCK_MONOTONIC_RAW, std::ptr::addr_of_mut!(timespec))
                == -1
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
        pub fn ftruncate(fd: std::ffi::c_int, length: std::ffi::c_int) -> std::ffi::c_int;
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
    const CLOCK_MONOTONIC_RAW: i32 = 4;
}

mod wl {
    use *;

    const WL_DISPLAY_GET_REGISTRY: u32 = 1;

    const WL_COMPOSITOR_CREATE_SURFACE: u32 = 0;

    const WL_REGISTRY_BIND: u32 = 0;

    const WL_SURFACE_ATTACH: u32 = 1;
    const WL_SURFACE_COMMIT: u32 = 6;
    const WL_SURFACE_DAMAGE_BUFFER: u32 = 9;

    const WL_SHM_CREATE_POOL: u32 = 0;

    const WL_SHM_POOL_CREATE_BUFFER: u32 = 0;
    const WL_SHM_POOL_DESTROY: u32 = 1;

    const WL_BUFFER_DESTROY: u32 = 0;

    const WL_MARSHAL_FLAG_DESTROY: u32 = 1;

    const WL_SEAT_GET_KEYBOARD: u32 = 1;

    const WL_KEYBOARD_RELEASE: u32 = 0;

    pub enum SHMFormat {
        XRGB8888 = 1,
    }

    #[allow(dead_code)]
    enum SeatCapability {
        POINTER = 1,
        KEYBOARD = 2,
        TOUCH = 4,
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

    #[allow(dead_code)]
    pub fn display_dispatch(display: *mut wl_display) -> i32 {
        unsafe { wl_display_dispatch(display) }
    }

    #[allow(dead_code)]
    pub fn display_dispatch_pending(display: *mut wl_display) -> i32 {
        unsafe {
            while wl_display_prepare_read(display) != 0 {
                wl_display_dispatch_pending(display);
            }
            wl_display_flush(display);

            wl_display_read_events(display);
            wl_display_dispatch_pending(display)
        }
    }

    pub fn display_dispatch_pending_single(display: *mut wl_display) -> i32 {
        unsafe {
            while wl_display_prepare_read(display) != 0 {
                wl_display_dispatch_pending_single(display);
            }
            wl_display_flush(display);

            const POLLIN: i16 = 0x001;
            let mut fds = posix::Pollfd {
                fd: wl_display_get_fd(display),
                events: POLLIN,
                revents: 0,
            };
            let nfds = 1;
            if posix::poll(&mut fds, nfds, -1) == -1 {
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

    pub fn registry_add_listener(registry: *mut wl_registry, global_state: *mut unix::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                registry as *mut wl_proxy,
                std::ptr::addr_of_mut!(registry_listener).cast::<unix::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn buffer_add_listener(buffer: *mut wl_buffer, global_state: *mut unix::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                buffer as *mut wl_proxy,
                std::ptr::addr_of_mut!(buffer_listener).cast::<unix::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    #[allow(dead_code)]
    pub fn buffer_destroy(buffer: *mut wl_buffer) {
        let proxy = buffer as *mut wl_proxy;
        unsafe {
            wl_proxy_marshal_array_flags(
                proxy,
                WL_BUFFER_DESTROY,
                std::ptr::null(),
                wl_proxy_get_version(proxy),
                WL_MARSHAL_FLAG_DESTROY,
                std::ptr::null_mut() as *mut wl_argument,
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
            implementation: *mut unix::ListenerImplementation,
            data: *mut std::ffi::c_void,
        ) -> std::ffi::c_int;

        fn wl_display_roundtrip(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_connect(name: *const std::ffi::c_char) -> *mut wl_display;
        fn wl_display_disconnect(display: *mut wl_display);
        fn wl_display_dispatch(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_dispatch_pending(display: *mut wl_display) -> std::ffi::c_int;
        fn wl_display_dispatch_pending_single(display: *mut wl_display) -> std::ffi::c_int;
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
        let global_state = &mut *data.cast::<unix::GlobalState>();
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
                data.cast::<unix::GlobalState>(),
            );
        } else if interface == std::ffi::CStr::from_ptr(wl_seat_interface.name) {
            global_state.seat =
                registry_bind(registry, name, &wl_seat_interface, version) as *mut wl::wl_seat;
            seat_add_listener(global_state.seat, data.cast::<unix::GlobalState>());
        }
    }

    pub unsafe extern "C" fn buffer_release(data: *mut std::ffi::c_void, _buffer: *mut wl_buffer) {
        let global_state = &mut *data.cast::<unix::GlobalState>();
        global_state.buffer_released = true;
    }

    pub unsafe extern "C" fn seat_capabilities(
        data: *mut std::ffi::c_void,
        _seat: *mut wl_seat,
        capabilities: std::ffi::c_uint,
    ) {
        let global_state = &mut *data.cast::<unix::GlobalState>();
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
        let global_state = &mut *data.cast::<unix::GlobalState>();
        global_state.event = unix::EventType::Keyboard(key, key_state);
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

    fn seat_add_listener(seat: *mut wl_seat, global_state: *mut unix::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                seat as *mut wl_proxy,
                std::ptr::addr_of_mut!(seat_listener).cast::<unix::ListenerImplementation>(),
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

    fn keyboard_add_listener(keyboard: *mut wl_keyboard, global_state: *mut unix::GlobalState) {
        unsafe {
            wl_proxy_add_listener(
                keyboard as *mut wl_proxy,
                std::ptr::addr_of_mut!(keyboard_listener).cast::<unix::ListenerImplementation>(),
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
    use *;

    const XDG_WM_BASE_GET_XDG_SURFACE: u32 = 2;
    const XDG_WM_BASE_PONG: u32 = 3;

    const XDG_SURFACE_GET_TOPLEVEL: u32 = 1;
    const XDG_SURFACE_ACK_CONFIGURE: u32 = 4;

    const XDG_TOPLEVEL_SET_TITLE: u32 = 2;

    pub fn wm_add_listener(wm: *mut xdg_wm_base, global_state: *mut unix::GlobalState) {
        unsafe {
            wl::wl_proxy_add_listener(
                wm as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(wm_listener).cast::<unix::ListenerImplementation>(),
                global_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn surface_add_listener(surface: *mut xdg_surface, global_state: *mut unix::GlobalState) {
        unsafe {
            wl::wl_proxy_add_listener(
                surface as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(surface_listener).cast::<unix::ListenerImplementation>(),
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
        global_state: *mut unix::GlobalState,
    ) {
        unsafe {
            wl::wl_proxy_add_listener(
                toplevel as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(toplevel_listener).cast::<unix::ListenerImplementation>(),
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
        let global_state = &mut *data.cast::<unix::GlobalState>();
        unix::resize_shared_buffer(global_state, width, height);
    }

    unsafe extern "C" fn toplevel_close(data: *mut std::ffi::c_void, _toplevel: *mut xdg_toplevel) {
        let global_state = &mut *data.cast::<unix::GlobalState>();
        global_state.event = unix::EventType::Close;
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
    use *;

    const PW_VERSION_STREAM_EVENTS: u32 = 2;

    pub fn init(sound_output: &mut unix::SoundOutput) {
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
                sound_output as *mut unix::SoundOutput as *mut std::ffi::c_void,
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
        let sound_output = &mut *userdata.cast::<unix::SoundOutput>();

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
        let samples =
            std::slice::from_raw_parts_mut(buffers[0].data as *mut u8, buffer_size as usize)
                .chunks_exact_mut(sound_output.bytes_per_sample as usize);
        for sample in samples {
            let sample_value = sound_output.t_sine.sin() * sound_output.tone_volume;
            std::ptr::copy_nonoverlapping(
                [sample_value; 2].as_ptr() as *mut u8,
                sample.as_ptr() as *mut u8,
                sound_output.bytes_per_sample as usize,
            );
            sound_output.t_sine += 2 as f32 * std::f32::consts::PI * 1.0 / sound_output.wave_period;
        }

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
    #[allow(dead_code)]
    #[repr(C)]
    enum spa_audio_format {
        S16 = 0x104,
        F32 = 0x11B,
        S32 = 0x11C,
    }
    #[allow(dead_code)]
    #[repr(C)]
    enum spa_direction {
        Input,
        Output = 1,
    }
    enum StreamFlag {
        AutoConnect = 1 << 0,
        MapBuffers = 1 << 2,
    }
}

fn main() {
    let mut global_state: unix::GlobalState = unix::GlobalState::default();
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
        unix::resize_shared_buffer(&mut global_state, 1280, 720);

        let mut x_offset = 0;
        let mut y_offset = 0;

        let mut sound_output = unix::SoundOutput {
            samples_per_second: 48000,
            bytes_per_sample: 0,
            channels: 2,
            tone_hz: 256,
            tone_volume: 0.2,
            wave_period: 0.,
            sound_main_loop: std::ptr::null_mut(),
            sound_loop: std::ptr::null_mut(),
            stream: std::ptr::null_mut(),
            t_sine: 0.,
        };
        sound_output.bytes_per_sample = sound_output.channels * std::mem::size_of::<f32>() as u32;
        sound_output.wave_period =
            sound_output.samples_per_second as f32 / sound_output.tone_hz as f32;
        pw::init(&mut sound_output);
        pw::loop_enter(sound_output.sound_loop);

        let mut last_timestamp = posix::clock_get_time().unwrap();
        let mut last_cycle_count = posix::cycle_get_count();

        loop {
            if wl::display_dispatch_pending_single(display) == -1 {
                break;
            }

            match global_state.event {
                unix::EventType::Close => break,
                unix::EventType::Keyboard(key, key_state) => {
                    if key == unix::KeyCode::W as u32 {
                    } else if key == unix::KeyCode::A as u32 {
                    } else if key == unix::KeyCode::S as u32 {
                    } else if key == unix::KeyCode::D as u32 {
                    } else if key == unix::KeyCode::Q as u32 {
                    } else if key == unix::KeyCode::E as u32 {
                    } else if key == unix::KeyCode::UP as u32 {
                        y_offset += 2;
                        sound_output.tone_hz = 512 + (x_offset as i32).rem_euclid(512) as u32;
                    } else if key == unix::KeyCode::LEFT as u32 {
                        x_offset -= 2;
                    } else if key == unix::KeyCode::DOWN as u32 {
                        y_offset -= 2;
                        sound_output.tone_hz = 512 - (x_offset as i32).rem_euclid(512) as u32;
                    } else if key == unix::KeyCode::RIGHT as u32 {
                        x_offset += 2;
                    } else if key == unix::KeyCode::SPACE as u32 {
                    } else if key == unix::KeyCode::ESC as u32 {
                        if key_state == 0 {
                            println!("esc is not pressed");
                        }
                        if key_state == 1 {
                            println!("esc is pressed");
                        }
                    }
                }
                _ => {}
            }
            sound_output.wave_period =
                sound_output.samples_per_second as f32 / sound_output.tone_hz as f32;

            if global_state.buffer_released {
                render_weird_gradient(x_offset, y_offset, &mut global_state.back_buffer);
                unix::display_buffer_in_window(&mut global_state, 0, 0);
            }

            pw::loop_iterate(sound_output.sound_loop);

            global_state.event = unix::EventType::None;

            let end_cycle_count = posix::cycle_get_count();
            let end_timestamp = posix::clock_get_time().unwrap();

            let cycles_elapsed = end_cycle_count - last_cycle_count;
            let ms_per_frame = (end_timestamp - last_timestamp) * 1e3;
            let fps = 1e3 / ms_per_frame;
            let mcpf = cycles_elapsed as f64 / 1e6;
            println!("{:.02}ms/f, {:.02}f/s, {:.02}mc/f", ms_per_frame, fps, mcpf);

            last_cycle_count = end_cycle_count;
            last_timestamp = end_timestamp;
        }
        pw::loop_leave(sound_output.sound_loop);
        wl::display_disconnect(display);
    } else {
        panic!("display_connect.");
    }
}
