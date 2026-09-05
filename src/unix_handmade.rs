fn render_weird_gradient(x_offset: i32, y_offset: i32, buffer: &mut unix::OffscreenBuffer) {
    let rows = buffer
        .memory
        .as_slice_mut()
        .chunks_exact_mut(buffer.pitch as usize)
        .enumerate();
    for (y, row) in rows {
        let pixels = row
            .chunks_exact_mut(buffer.bytes_per_pixel as usize)
            .enumerate();
        for (x, pixel) in pixels {
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

    impl Default for EventType {
        fn default() -> Self {
            EventType::None
        }
    }

    #[derive(Default)]
    pub struct OffscreenBuffer {
        pub memory: posix::MemFd,
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
        pub bytes_per_pixel: i32,
    }

    pub fn resize_shared_buffer(client_state: &mut wl::ClientState, width: i32, height: i32) {
        let buffer = &mut client_state.back_buffer;
        buffer.width = width;
        buffer.height = height;
        buffer.pitch = width * buffer.bytes_per_pixel;
        let bitmap_size = height * buffer.pitch;
        if !buffer.memory.is_null() {
            posix::memfd_release(&mut buffer.memory);
        }

        buffer.memory = posix::memfd_alloc("handmade_hero", bitmap_size);
        let pool = wl::shm_create_pool(client_state.shm, buffer.memory.fd, bitmap_size);
        client_state.buffer = wl::shm_pool_create_buffer(
            pool,
            0,
            buffer.width,
            buffer.height,
            buffer.pitch,
            wl::SHMFormat::XRGB8888 as u32,
        );
        wl::shm_pool_destroy(pool);
        wl::buffer_add_listener(client_state.buffer, client_state);
    }

    pub fn display_buffer_in_window(client_state: &mut wl::ClientState, x: i32, y: i32) {
        client_state.buffer_released = false;
        wl::surface_damage_buffer(
            client_state.surface,
            x,
            y,
            client_state.back_buffer.width,
            client_state.back_buffer.height,
        );
        wl::surface_attach(client_state.surface, client_state.buffer, x, y);
        wl::surface_commit(client_state.surface);
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

        let mut buf = [0u8; 260];
        assert!(
            !name.is_empty() && name.len() < buf.len(),
            "allocate_memory"
        );

        buf[..name.len()].copy_from_slice(&name.as_bytes());
        let fd = unsafe { posix::memfd_create(buf.as_ptr() as *const i8, MFD_CLOEXEC) };
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
    }

    #[repr(C)]
    pub struct Pollfd {
        pub fd: std::ffi::c_int,
        pub events: std::ffi::c_short,
        pub revents: std::ffi::c_short,
    }
}

mod wl {
    use *;

    #[derive(Default)]
    pub struct ClientState {
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
        pub running: bool,
        pub buffer_released: bool,
        pub back_buffer: unix::OffscreenBuffer,
    }

    pub type ListenerImplementation = unsafe extern "C" fn();

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

    enum SeatCapability {
        // POINTER = 1,
        KEYBOARD = 2,
        // TOUCH = 4,
    }

    pub fn display_connect(sock_name: &str) -> Option<*mut wl_display> {
        let name = if sock_name.is_empty() {
            std::ptr::null()
        } else {
            let mut buf = [0u8; 260];
            if sock_name.len() >= buf.len() {
                return None;
            }

            buf[..sock_name.len()].copy_from_slice(&sock_name.as_bytes());
            buf.as_ptr() as *const i8
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
                assert!(fds.revents == POLLIN);
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

    pub fn registry_add_listener(registry: *mut wl_registry, client_state: *mut ClientState) {
        unsafe {
            wl_proxy_add_listener(
                registry as *mut wl_proxy,
                std::ptr::addr_of_mut!(registry_listener).cast::<ListenerImplementation>(),
                client_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn buffer_add_listener(buffer: *mut wl_buffer, client_state: *mut ClientState) {
        unsafe {
            wl_proxy_add_listener(
                buffer as *mut wl_proxy,
                std::ptr::addr_of_mut!(buffer_listener).cast::<ListenerImplementation>(),
                client_state as *mut std::ffi::c_void,
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
            implementation: *mut ListenerImplementation,
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
        let interface = std::ffi::CStr::from_ptr(interface).to_str().unwrap_or("");

        let client_state = &mut *data.cast::<ClientState>();
        if interface
            == std::ffi::CStr::from_ptr(wl_compositor_interface.name)
                .to_str()
                .unwrap()
        {
            client_state.compositor =
                registry_bind(registry, name, &wl_compositor_interface, version)
                    as *mut wl_compositor;
        } else if interface
            == std::ffi::CStr::from_ptr(wl_shm_interface.name)
                .to_str()
                .unwrap()
        {
            client_state.shm =
                registry_bind(registry, name, &wl_shm_interface, version) as *mut wl_shm;
        } else if interface
            == std::ffi::CStr::from_ptr(xdg::xdg_wm_base_interface.name)
                .to_str()
                .unwrap()
        {
            client_state.window_manager =
                registry_bind(registry, name, &xdg::xdg_wm_base_interface, version)
                    as *mut xdg::xdg_wm_base;
            xdg::wm_add_listener(client_state.window_manager, data.cast::<ClientState>());
        } else if interface
            == std::ffi::CStr::from_ptr(wl_seat_interface.name)
                .to_str()
                .unwrap()
        {
            client_state.seat =
                registry_bind(registry, name, &wl_seat_interface, version) as *mut wl::wl_seat;
            seat_add_listener(client_state.seat, data.cast::<ClientState>());
        }
    }

    pub unsafe extern "C" fn buffer_release(data: *mut std::ffi::c_void, _buffer: *mut wl_buffer) {
        let client_state = &mut *data.cast::<ClientState>();
        client_state.buffer_released = true;
    }

    pub unsafe extern "C" fn seat_capabilities(
        data: *mut std::ffi::c_void,
        _seat: *mut wl_seat,
        capabilities: std::ffi::c_uint,
    ) {
        let client_state = &mut *data.cast::<wl::ClientState>();
        let has_keyboard = (capabilities & SeatCapability::KEYBOARD as u32) != 0;
        if has_keyboard && client_state.keyboard.is_null() {
            client_state.keyboard = seat_get_keyboard(client_state.seat);
            keyboard_add_listener(client_state.keyboard, client_state);
        } else if !has_keyboard && !client_state.keyboard.is_null() {
            keyboard_release(client_state.keyboard);
            client_state.keyboard = std::ptr::null_mut();
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
        let client_state = &mut *data.cast::<ClientState>();
        client_state.event = unix::EventType::Keyboard(key, key_state);
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

    fn seat_add_listener(seat: *mut wl_seat, client_state: *mut ClientState) {
        unsafe {
            wl_proxy_add_listener(
                seat as *mut wl_proxy,
                std::ptr::addr_of_mut!(seat_listener).cast::<ListenerImplementation>(),
                client_state as *mut std::ffi::c_void,
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

    fn keyboard_add_listener(keyboard: *mut wl_keyboard, client_state: *mut ClientState) {
        unsafe {
            wl_proxy_add_listener(
                keyboard as *mut wl_proxy,
                std::ptr::addr_of_mut!(keyboard_listener).cast::<ListenerImplementation>(),
                client_state as *mut std::ffi::c_void,
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

    pub fn wm_add_listener(wm: *mut xdg_wm_base, client_state: *mut wl::ClientState) {
        unsafe {
            wl::wl_proxy_add_listener(
                wm as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(wm_listener).cast::<wl::ListenerImplementation>(),
                client_state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn surface_add_listener(surface: *mut xdg_surface, client_state: *mut wl::ClientState) {
        unsafe {
            wl::wl_proxy_add_listener(
                surface as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(surface_listener).cast::<wl::ListenerImplementation>(),
                client_state as *mut std::ffi::c_void,
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
            let mut buf = [0u8; 260];
            if title.len() >= buf.len() {
                return;
            }

            buf[..title.len()].copy_from_slice(&title.as_bytes());
            buf.as_ptr() as *const i8
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

    pub fn toplevel_add_listener(toplevel: *mut xdg_toplevel, client_state: *mut wl::ClientState) {
        unsafe {
            wl::wl_proxy_add_listener(
                toplevel as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(toplevel_listener).cast::<wl::ListenerImplementation>(),
                client_state as *mut std::ffi::c_void,
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
        let client_state = &mut *data.cast::<wl::ClientState>();
        unix::resize_shared_buffer(client_state, width, height);
    }

    unsafe extern "C" fn toplevel_close(data: *mut std::ffi::c_void, _toplevel: *mut xdg_toplevel) {
        let client_state = &mut *data.cast::<wl::ClientState>();
        client_state.event = unix::EventType::Close;
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

mod pipewire {
    use *;

    const PW_VERSION_STREAM_EVENTS: u32 = 2;

    const PW_ID_ANY: u32 = 0xffffffff;

    const PW_KEY_MEDIA_TYPE: &str = "media.type\0";
    const PW_KEY_MEDIA_CATEGORY: &str = "media.category\0";
    const PW_KEY_MEDIA_ROLE: &str = "media.role\0";

    const M_PI_M2: f64 = std::f64::consts::PI + std::f64::consts::PI;
    const DEFAULT_RATE: f64 = 44100.;
    const DEFAULT_VOLUME: f64 = 0.7;
    const DEFAULT_CHANNELS: u32 = 2;

    const SPA_AUDIO_MAX_CHANNELS: usize = 64;

    macro_rules! SPA_POD_BUILDER_INIT {
        ($buffer:expr) => {
            spa_pod_builder {
                data: $buffer.as_mut_ptr() as *mut std::ffi::c_void,
                size: $buffer.len() as u32,
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
            }
        };
    }

    pub fn init() {
        unsafe {
            pw_init(std::ptr::null_mut(), std::ptr::null_mut());

            let mut buffer = [0u8; 1024];
            let mut pod_builder = SPA_POD_BUILDER_INIT!(buffer);

            let mut property_items = [
                spa_dict_item {
                    key: PW_KEY_MEDIA_TYPE.as_ptr() as *const i8,
                    value: "Audio\0".as_ptr() as *const i8,
                },
                spa_dict_item {
                    key: PW_KEY_MEDIA_CATEGORY.as_ptr() as *const i8,
                    value: "Playback\0".as_ptr() as *const i8,
                },
                spa_dict_item {
                    key: PW_KEY_MEDIA_ROLE.as_ptr() as *const i8,
                    value: "Music\0".as_ptr() as *const i8,
                },
            ];
            let properties = spa_dict {
                flags: 0,
                n_items: 3,
                items: property_items.as_mut_ptr(),
            };
            let mut data = Data::default();
            data.main_loop = pw_main_loop_new(std::ptr::null_mut());
            data.stream = pw_stream_new_simple(
                pw_main_loop_get_loop(data.main_loop),
                "handmade-audio\0".as_ptr() as *const i8,
                pw_properties_new_dict(&properties as *const spa_dict),
                std::ptr::addr_of_mut!(stream_events),
                std::ptr::addr_of_mut!(data) as *mut std::ffi::c_void,
            );

            let params = [spa_format_audio_raw_build(
                &mut pod_builder,
                spa_param_type::EnumFormat as u32,
                &spa_audio_info_raw {
                    format: spa_audio_format::S16,
                    flags: 0,
                    channels: DEFAULT_CHANNELS,
                    rate: DEFAULT_RATE as u32,
                    position: [0; SPA_AUDIO_MAX_CHANNELS],
                },
            )];
            pw_stream_connect(
                data.stream,
                spa_direction::Output,
                PW_ID_ANY,
                PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS | PW_STREAM_FLAG_RT_PROCESS,
                params.as_ptr(),
                1,
            );

            pw_main_loop_run(data.main_loop);
        }
    }

    #[derive(Default)]
    struct Data {
        main_loop: *mut pw_main_loop,
        stream: *mut pw_stream,
        accumulator: f64,
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
        process: Some(on_process),
        drained: None,
        command: None,
        trigger_done: None,
    };

    #[repr(C)]
    struct pw_loop {
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

    unsafe extern "C" fn on_process(userdata: *mut std::ffi::c_void) {
        let data = &mut *userdata.cast::<Data>();

        let b = pw_stream_dequeue_buffer(data.stream);
        if b.is_null() {
            println!("out of buffers");
            return;
        }

        let datas = &mut *(*(*b).buffer).datas;
        if datas.data.is_null() {
            return;
        }
        let dst = datas.data as *mut f64;

        let stride = std::mem::size_of::<i16>() as u32 * DEFAULT_CHANNELS;
        let mut n_frames = datas.maxsize / stride;
        let requested = (*b).requested as u32;
        if requested != 0 {
            n_frames = std::cmp::min(n_frames, requested);
        }

        for _ in 0..n_frames {
            data.accumulator += M_PI_M2 * 440. / DEFAULT_RATE;
            if data.accumulator >= M_PI_M2 {
                data.accumulator -= M_PI_M2;
            }

            let val = f64::sin(data.accumulator) * DEFAULT_VOLUME * 32767.;
            for _ in 0..DEFAULT_CHANNELS {
                *dst = val;
                let _ = dst.wrapping_add(1);
            }
        }

        let chunk = &mut *(datas.chunk);
        chunk.offset = 0;
        chunk.stride = stride as i32;
        chunk.size = n_frames * stride;

        pw_stream_queue_buffer(data.stream, b);
    }

    #[link(name = "pipewire-0.3")]
    unsafe extern "C" {
        fn pw_init(argc: *mut std::ffi::c_int, argv: *mut *mut std::ffi::c_char);

        fn pw_main_loop_new(props: *const spa_dict) -> *mut pw_main_loop;
        fn pw_main_loop_get_loop(main_loop: *mut pw_main_loop) -> *mut pw_loop;
        fn pw_main_loop_run(main_loop: *mut pw_main_loop) -> std::ffi::c_int;

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
    }
    #[repr(C)]
    pub struct pw_main_loop {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct pw_properties {
        _data: (),
        _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    pub struct pw_stream {
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
    struct spa_system {
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
        position: [std::ffi::c_uint; SPA_AUDIO_MAX_CHANNELS],
    }
    #[repr(C)]
    enum spa_audio_format {
        S16 = 0x104,
    }
    #[allow(dead_code)]
    #[repr(C)]
    enum spa_direction {
        Input,
        Output = 1,
    }
    const PW_STREAM_FLAG_AUTOCONNECT: u32 = 1 << 0;
    const PW_STREAM_FLAG_MAP_BUFFERS: u32 = 1 << 2;
    const PW_STREAM_FLAG_RT_PROCESS: u32 = 1 << 4;
}

fn main() {
    pipewire::init();

    let mut client_state: wl::ClientState = wl::ClientState::default();
    client_state.running = true;
    client_state.buffer_released = true;
    client_state.back_buffer.bytes_per_pixel = std::mem::size_of::<i32>() as i32;

    if let Some(display) = wl::display_connect("") {
        let registry = wl::display_get_registry(display);
        wl::registry_add_listener(registry, &mut client_state);
        wl::display_roundtrip(display);

        assert!(!client_state.compositor.is_null());
        client_state.surface = wl::compositor_create_surface(client_state.compositor);
        client_state.window =
            xdg::wm_get_xdg_surface(client_state.window_manager, client_state.surface);
        xdg::surface_add_listener(client_state.window, &mut client_state);

        client_state.toplevel = xdg::surface_get_toplevel(client_state.window);
        xdg::toplevel_set_title(client_state.toplevel, "Handmade Hero");
        xdg::toplevel_add_listener(client_state.toplevel, &mut client_state);

        wl::surface_commit(client_state.surface);
        unix::resize_shared_buffer(&mut client_state, 1280, 720);

        let mut x_offset = 0;
        let mut y_offset = 0;
        while client_state.running {
            let event_count = wl::display_dispatch_pending_single(display);
            if event_count != -1 {
                if client_state.buffer_released {
                    render_weird_gradient(x_offset, y_offset, &mut client_state.back_buffer);
                    unix::display_buffer_in_window(&mut client_state, 0, 0);
                    x_offset += 1;
                    y_offset += 2;
                }
            } else {
                assert!(event_count == 1);
                client_state.running = false;
            }

            match client_state.event {
                unix::EventType::Close => client_state.running = false,
                unix::EventType::Keyboard(key, key_state) => {
                    if key == unix::KeyCode::W as u32 {
                    } else if key == unix::KeyCode::A as u32 {
                    } else if key == unix::KeyCode::S as u32 {
                    } else if key == unix::KeyCode::D as u32 {
                    } else if key == unix::KeyCode::Q as u32 {
                    } else if key == unix::KeyCode::E as u32 {
                    } else if key == unix::KeyCode::UP as u32 {
                    } else if key == unix::KeyCode::LEFT as u32 {
                    } else if key == unix::KeyCode::DOWN as u32 {
                    } else if key == unix::KeyCode::RIGHT as u32 {
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

            client_state.event = unix::EventType::None;
        }
        wl::display_disconnect(display);
    } else {
        panic!("display_connect.");
    }
}
