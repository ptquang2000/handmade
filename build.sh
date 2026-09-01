ROOT=$(pwd)
mkdir -p build
pushd build >/dev/null
wayland-scanner private-code /usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml xdg-shell-protocol.c
wayland-scanner client-header /usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml xdg-shell-client-protocol.h
gcc -c xdg-shell-protocol.c -o xdg-shell-protocol.o
ar rcs libxdg-shell-protocol.a xdg-shell-protocol.o
rustc -g -C opt-level=0 -L . --out-dir . $ROOT/src/unix_handmade.rs
popd >/dev/null
