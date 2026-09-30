ROOT=$(pwd)
COMPILER_FLAGS='--cfg HANDMADE_INTERNAL -g -C opt-level=0'
LINK_FLAGS='-C link-args=-Wl,-rpath,/usr/lib/spa-0.2 -C link-args=-Wl,-rpath,$ORIGIN -L /usr/lib/spa-0.2 -L .'

XDG_LIB_PATH='/usr/share/wayland-protocols/stable/xdg-shell'
ALPHA_MOD_PATH='/usr/share/wayland-protocols/staging/alpha-modifier'

mkdir -p build
pushd build >/dev/null

wayland-scanner private-code ${XDG_LIB_PATH}/xdg-shell.xml xdg-shell-protocol.c
wayland-scanner client-header ${XDG_LIB_PATH}/xdg-shell.xml xdg-shell-client-protocol.h
gcc -c xdg-shell-protocol.c -o xdg-shell-protocol.o
ar rcs libxdg-shell-protocol.a xdg-shell-protocol.o

wayland-scanner private-code  ${ALPHA_MOD_PATH}/alpha-modifier-v1.xml alpha-modifier-protocol.c
wayland-scanner client-header ${ALPHA_MOD_PATH}/alpha-modifier-v1.xml alpha-modifier-client-protocol.h
gcc -c alpha-modifier-protocol.c -o alpha-modifier-protocol.o
ar rcs libalpha-modifier-protocol.a alpha-modifier-protocol.o

rustc $COMPILER_FLAGS $LINK_FLAGS --out-dir . $ROOT/src/handmade.rs --crate-type cdylib
rustc $COMPILER_FLAGS $LINK_FLAGS --out-dir . $ROOT/src/linux_handmade.rs --crate-type bin

popd >/dev/null
