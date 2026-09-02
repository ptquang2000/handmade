struct ClientState {
    compositor: *mut wl::wl_compositor,
    shm: *mut wl::wl_shm,
    surface: *mut wl::wl_surface,

    window_manager: *mut xdg::xdg_wm_base,
    window: *mut xdg::xdg_surface,
    toplevel: *mut xdg::xdg_toplevel,

    running: bool,
    bitmap: Buffer,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            compositor: std::ptr::null_mut(),
            shm: std::ptr::null_mut(),
            surface: std::ptr::null_mut(),

            window_manager: std::ptr::null_mut(),
            window: std::ptr::null_mut(),
            toplevel: std::ptr::null_mut(),

            running: true,
            bitmap: Buffer {
                addr: std::ptr::null_mut(),
                fd: 0,
                size: 0,
            },
        }
    }
}

pub struct Buffer {
    addr: *mut u32,
    fd: i32,
    size: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            addr: std::ptr::null_mut(),
            fd: -1,
            size: 0,
        }
    }
}

mod unix {
    use Buffer;

    pub fn allocate_memory(name: &str, size: i32) -> Buffer {
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
        let fd = unsafe { memfd_create(buf.as_ptr() as *const i8, MFD_CLOEXEC) };
        assert!(fd != -1, "memfd_create");
        let result = unsafe { ftruncate(fd, size) };
        assert!(result != -1, "ftruncate");
        let addr = unsafe {
            mmap(
                std::ptr::null_mut(),
                size as usize,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            ) as *mut ()
        };
        assert!(addr as i32 != MAP_FAILED, "mmap");
        Buffer {
            addr: addr as *mut u32,
            fd: fd,
            size: size as usize,
        }
    }

    pub fn release_memory(data: &mut Buffer) {
        unsafe { close(data.fd) };
        unsafe { munmap(data.addr as *mut std::ffi::c_void, data.size as usize) };
        *data = Buffer::default();
    }

    #[link(name = "c")]
    unsafe extern "C" {
        fn memfd_create(name: *const std::ffi::c_char, oflag: std::ffi::c_uint) -> std::ffi::c_int;
        fn close(fd: std::ffi::c_int) -> std::ffi::c_int;
        fn ftruncate(fd: std::ffi::c_int, length: std::ffi::c_int) -> std::ffi::c_int;
        fn mmap(
            addr: *const std::ffi::c_void,
            length: usize,
            prot: std::ffi::c_int,
            flags: std::ffi::c_int,
            fd: std::ffi::c_int,
            offset: std::ffi::c_int,
        ) -> *mut std::ffi::c_void;
        fn munmap(addr: *mut std::ffi::c_void, len: usize) -> std::ffi::c_int;
    }
}

mod wl {
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

    pub enum SHMFormat {
        XRGB8888 = 1,
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

    pub fn display_dispatch(display: *mut wl_display) -> i32 {
        unsafe { wl_display_dispatch(display) }
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

    pub fn registry_add_listener(registry: *mut wl_registry, state: *mut ClientState) {
        unsafe {
            wl_proxy_add_listener(
                registry as *mut wl_proxy,
                std::ptr::addr_of_mut!(registry_listener).cast::<ListenerImplementation>(),
                state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn buffer_add_listener(buffer: *mut wl_buffer) {
        unsafe {
            wl_proxy_add_listener(
                buffer as *mut wl_proxy,
                std::ptr::addr_of_mut!(buffer_listener).cast::<ListenerImplementation>(),
                std::ptr::null_mut(),
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

    pub fn shm_create_pool(
        shm: *mut wl_shm,
        fd: std::ffi::c_int,
        size: std::ffi::c_int,
    ) -> *mut wl_shm_pool {
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

    use {xdg, ClientState};

    #[link(name = "wayland-client")]
    unsafe extern "C" {
        static wl_registry_interface: wl_interface;
        static wl_compositor_interface: wl_interface;
        static wl_surface_interface: wl_interface;
        static wl_shm_interface: wl_interface;
        static wl_shm_pool_interface: wl_interface;
        static wl_buffer_interface: wl_interface;

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
        global: RegistryGlobal,
        global_remove: RegistryGlobalRemove,
    }
    #[repr(C)]
    struct wl_buffer_listener {
        release: BufferRelease,
    }
    type RegistryGlobal = unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *mut wl_registry,
        std::ffi::c_uint,
        *const std::ffi::c_char,
        std::ffi::c_uint,
    );
    type RegistryGlobalRemove =
        unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_registry, std::ffi::c_uint);
    type BufferRelease = unsafe extern "C" fn(*mut std::ffi::c_void, *mut wl_buffer);
    #[no_mangle]
    static mut registry_listener: wl_registry_listener = wl_registry_listener {
        global: registry_global,
        global_remove: registry_global_remove,
    };
    #[no_mangle]
    static mut buffer_listener: wl_buffer_listener = wl_buffer_listener {
        release: buffer_release,
    };

    unsafe extern "C" fn registry_global(
        data: *mut std::ffi::c_void,
        registry: *mut wl_registry,
        name: std::ffi::c_uint,
        interface: *const std::ffi::c_char,
        version: std::ffi::c_uint,
    ) {
        let interface = std::ffi::CStr::from_ptr(interface).to_str().unwrap_or("");

        let state = data.cast::<ClientState>();
        if interface
            == std::ffi::CStr::from_ptr(wl_compositor_interface.name)
                .to_str()
                .unwrap()
        {
            (*state).compositor = registry_bind(registry, name, &wl_compositor_interface, version)
                as *mut wl_compositor;
        } else if interface
            == std::ffi::CStr::from_ptr(wl_shm_interface.name)
                .to_str()
                .unwrap()
        {
            (*state).shm = registry_bind(registry, name, &wl_shm_interface, version) as *mut wl_shm;
        } else if interface
            == std::ffi::CStr::from_ptr(xdg::xdg_wm_base_interface.name)
                .to_str()
                .unwrap()
        {
            (*state).window_manager =
                registry_bind(registry, name, &xdg::xdg_wm_base_interface, version)
                    as *mut xdg::xdg_wm_base;
            xdg::wm_add_listener((*state).window_manager, state);
        }
    }

    unsafe extern "C" fn registry_global_remove(
        _data: *mut std::ffi::c_void,
        _registry: *mut wl_registry,
        _name: std::ffi::c_uint,
    ) {
    }

    unsafe extern "C" fn buffer_release(_data: *mut std::ffi::c_void, buffer: *mut wl_buffer) {
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
}

mod xdg {
    const XDG_WM_BASE_GET_XDG_SURFACE: u32 = 2;
    const XDG_WM_BASE_PONG: u32 = 3;

    const XDG_SURFACE_GET_TOPLEVEL: u32 = 1;
    const XDG_SURFACE_ACK_CONFIGURE: u32 = 4;

    const XDG_TOPLEVEL_SET_TITLE: u32 = 2;

    pub fn wm_add_listener(wm: *mut xdg_wm_base, state: *mut ClientState) {
        unsafe {
            wl::wl_proxy_add_listener(
                wm as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(wm_listener).cast::<wl::ListenerImplementation>(),
                state as *mut std::ffi::c_void,
            );
        }
    }

    pub fn surface_add_listener(surface: *mut xdg_surface, state: *mut ClientState) {
        unsafe {
            wl::wl_proxy_add_listener(
                surface as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(surface_listener).cast::<wl::ListenerImplementation>(),
                state as *mut std::ffi::c_void,
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

    pub fn toplevel_add_listener(toplevel: *mut xdg_toplevel, state: *mut ClientState) {
        unsafe {
            wl::wl_proxy_add_listener(
                toplevel as *mut wl::wl_proxy,
                std::ptr::addr_of_mut!(toplevel_listener).cast::<wl::ListenerImplementation>(),
                state as *mut std::ffi::c_void,
            );
        }
    }

    use {unix, wl, ClientState};

    #[no_mangle]
    pub static mut wm_listener: xdg_wm_base_listener = xdg_wm_base_listener { ping: wm_ping };
    #[no_mangle]
    pub static mut surface_listener: xdg_surface_listener = xdg_surface_listener {
        configure: surface_configure,
    };
    #[no_mangle]
    pub static mut toplevel_listener: xdg_toplevel_listener = xdg_toplevel_listener {
        configure: toplevel_configure,
        close: toplevel_close,
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
        ping: XDGWMBasePing,
    }
    #[repr(C)]
    pub struct xdg_surface_listener {
        configure: XDGSurfaceConfigure,
    }
    #[repr(C)]
    pub struct xdg_toplevel_listener {
        configure: XDGToplevelConfigure,
        close: XDGToplevelClose,
    }
    type XDGWMBasePing =
        unsafe extern "C" fn(*mut std::ffi::c_void, *mut xdg_wm_base, std::ffi::c_uint);
    type XDGSurfaceConfigure =
        unsafe extern "C" fn(*mut std::ffi::c_void, *mut xdg_surface, std::ffi::c_uint);
    type XDGToplevelConfigure = unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *mut xdg_toplevel,
        std::ffi::c_int,
        std::ffi::c_int,
        *mut wl::wl_array,
    );
    type XDGToplevelClose = unsafe extern "C" fn(*mut std::ffi::c_void, *mut xdg_toplevel);

    unsafe extern "C" fn wm_ping(
        _data: *mut std::ffi::c_void,
        xdg_wm_base: *mut xdg_wm_base,
        serial: std::ffi::c_uint,
    ) {
        wm_pong(xdg_wm_base, serial);
    }

    unsafe extern "C" fn surface_configure(
        data: *mut std::ffi::c_void,
        surface: *mut xdg_surface,
        serial: std::ffi::c_uint,
    ) {
        let state = data.cast::<ClientState>();
        surface_ack_configure(surface, serial);

        wl::surface_commit((*state).surface);
    }

    unsafe extern "C" fn toplevel_configure(
        data: *mut std::ffi::c_void,
        _toplevel: *mut xdg_toplevel,
        width: std::ffi::c_int,
        height: std::ffi::c_int,
        _states: *mut wl::wl_array,
    ) {
        let state = data.cast::<ClientState>();
        let stride: i32 = width * std::mem::size_of_val(&width) as i32;
        (*state).bitmap = unix::allocate_memory("handmade_hero", height * stride);

        let pool = wl::shm_create_pool(
            (*state).shm,
            (*state).bitmap.fd,
            (*state).bitmap.size as i32,
        );
        let buffer = wl::shm_pool_create_buffer(
            pool,
            0,
            width,
            height,
            stride,
            wl::SHMFormat::XRGB8888 as u32,
        );
        wl::shm_pool_destroy(pool);

        unix::release_memory(&mut (*state).bitmap);
        wl::buffer_add_listener(buffer);

        wl::surface_attach((*state).surface, buffer, 0, 0);
        wl::surface_damage_buffer((*state).surface, 0, 0, width, height);
    }

    unsafe extern "C" fn toplevel_close(data: *mut std::ffi::c_void, _toplevel: *mut xdg_toplevel) {
        let state = data.cast::<ClientState>();
        (*state).running = false;
    }

    fn wm_pong(wm: *mut xdg_wm_base, serial: std::ffi::c_uint) {
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

    fn surface_ack_configure(surface: *mut xdg_surface, serial: std::ffi::c_uint) {
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

fn main() {
    let mut state: ClientState = ClientState::default();
    if let Some(display) = wl::display_connect("") {
        let registry = wl::display_get_registry(display);
        wl::registry_add_listener(registry, &mut state);
        wl::display_roundtrip(display);

        state.surface = wl::compositor_create_surface(state.compositor);
        state.window = xdg::wm_get_xdg_surface(state.window_manager, state.surface);
        xdg::surface_add_listener(state.window, &mut state);

        state.toplevel = xdg::surface_get_toplevel(state.window);
        xdg::toplevel_set_title(state.toplevel, "Handmade Hero");
        xdg::toplevel_add_listener(state.toplevel, &mut state);

        wl::surface_commit(state.surface);
        while wl::display_dispatch(display) != -1 {
            if !state.running {
                break;
            }
        }
        wl::display_disconnect(display)
    } else {
        panic!("display_connect.")
    }
}
