ROOT=$(pwd)
COMPILER_FLAGS='--cfg HANDMADE_INTERNAL -g -C opt-level=0'
LINK_FLAGS='-C link-args=-Wl,-rpath,/usr/lib/spa-0.2 -L /usr/lib/spa-0.2 -L .'

XDG_LIB_PATH='/usr/share/wayland-protocols/stable/xdg-shell'

mkdir -p build
pushd build >/dev/null

wayland-scanner private-code ${XDG_LIB_PATH}/xdg-shell.xml xdg-shell-protocol.c
wayland-scanner client-header ${XDG_LIB_PATH}/xdg-shell.xml xdg-shell-client-protocol.h
gcc -c xdg-shell-protocol.c -o xdg-shell-protocol.o
ar rcs libxdg-shell-protocol.a xdg-shell-protocol.o

rustc $COMPILER_FLAGS $LINK_FLAGS --out-dir . $ROOT/src/linux_handmade.rs

popd >/dev/null
