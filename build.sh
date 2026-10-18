ROOT=$(pwd)
COMPILER_FLAGS='--cfg HANDMADE_INTERNAL -g -C opt-level=0 -A unused'
LINK_FLAGS='-C link-args=-Wl,-rpath,/usr/lib/spa-0.2 -C link-args=-Wl,-rpath,$ORIGIN -L /usr/lib/spa-0.2 -L .'

XDG_LIB_PATH='/usr/share/wayland-protocols/stable/xdg-shell'
VIEWPORT_PATH='/usr/share/wayland-protocols/stable/viewporter'
ALPHA_MOD_PATH='/usr/share/wayland-protocols/staging/alpha-modifier'

mkdir -p build
pushd build >/dev/null

wayland-scanner private-code ${XDG_LIB_PATH}/xdg-shell.xml xdg-shell-protocol.c
wayland-scanner client-header ${XDG_LIB_PATH}/xdg-shell.xml xdg-shell-client-protocol.h
gcc -c xdg-shell-protocol.c -o xdg-shell-protocol.o
ar rcs libxdg-shell-protocol.a xdg-shell-protocol.o

wayland-scanner private-code  ${VIEWPORT_PATH}/viewporter.xml viewporter-protocol.c
wayland-scanner client-header ${VIEWPORT_PATH}/viewporter.xml viewporter-client-protocol.h
gcc -c viewporter-protocol.c -o viewporter-protocol.o
ar rcs libviewporter-protocol.a viewporter-protocol.o

wayland-scanner private-code  ${ALPHA_MOD_PATH}/alpha-modifier-v1.xml alpha-modifier-protocol.c
wayland-scanner client-header ${ALPHA_MOD_PATH}/alpha-modifier-v1.xml alpha-modifier-client-protocol.h
gcc -c alpha-modifier-protocol.c -o alpha-modifier-protocol.o
ar rcs libalpha-modifier-protocol.a alpha-modifier-protocol.o

echo WAITING FOR RDI > lock.tmp
rustc $COMPILER_FLAGS $LINK_FLAGS --out-dir . $ROOT/src/handmade.rs --crate-type cdylib
rm lock.tmp
rustc $COMPILER_FLAGS $LINK_FLAGS --out-dir . $ROOT/src/linux_handmade.rs --crate-type bin

popd >/dev/null
