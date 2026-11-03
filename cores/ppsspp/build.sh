#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"
# PPSSPP's libretro core, for OpenGL ES (the H700's Mali-G31 does 3.2) on arm64 Linux, with its
# ARM64 JIT. The release's own source tarball carries its submodules (glslang, SPIRV-Cross,
# ffmpeg's prebuilts and the rest), which GitHub's archive of a commit leaves out. The context
# is the frontend's, through libretro's hardware render interface: no EGL, X11, SDL or Vulkan.
recipe="cmake -DLIBRETRO=ON -DCMAKE_BUILD_TYPE=Release -DUSING_GLES2=ON -DUSING_EGL=OFF -DUSING_X11_VULKAN=OFF -DUSE_DISCORD=OFF -DUSE_MINIUPNPC=OFF -DUSE_WAYLAND_WSI=OFF -DUSE_VULKAN_DISPLAY_KHR=OFF -DHEADLESS=OFF -DUNITTEST=OFF"

usage() {
	echo "usage: $0 stamp VERSION | build VERSION TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "version=$1"
	echo "source=https://github.com/hrydgard/ppsspp/releases/download/v$1/ppsspp-$1.tar.xz"
	echo "recipe=$recipe"
	echo "device_cflags=$device_cflags"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

# Applies the patches with git kept out of any repository, as cores/pcsx_rearmed/build.sh does.
apply_patches() {
	ceiling="$(cd "$1/.." && pwd)"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		GIT_CEILING_DIRECTORIES="$ceiling" git -C "$1" apply -p1 "$p"
	done
}

# Builds the core from TARBALL into OUT, and its assets beside it as OUT's folder's PPSSPP/,
# which the core reads from the frontend's system folder.
build() {
	version="$1" tarball="$2" work="$3" out="$4"
	src="$work/ppsspp-$version"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xJf "$tarball" -C "$work"
	if [ ! -f "$src/CMakeLists.txt" ] || [ ! -d "$src/ext/glslang" ]; then
		echo "$tarball does not hold ppsspp-$version with its submodules" >&2
		exit 1
	fi

	apply_patches "$src"

	mkdir -p "$src/build"
	(cd "$src/build" &&
		CFLAGS="$device_cflags" CXXFLAGS="$device_cflags" $recipe .. &&
		make -j"$(getconf _NPROCESSORS_ONLN)" ppsspp_libretro)

	mkdir -p "$(dirname "$out")"
	lib="$(find "$src/build" -name ppsspp_libretro.so | head -n1)"
	[ -n "$lib" ] || {
		echo "the build left no ppsspp_libretro.so" >&2
		exit 1
	}
	cp "$lib" "$out"
	assets="$(dirname "$out")/PPSSPP"
	rm -rf "$assets"
	mkdir -p "$assets"
	cp -R "$src/assets/." "$assets/"
	stamp "$version" >"$out.meta"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	build "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
