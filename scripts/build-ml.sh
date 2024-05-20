#!/bin/bash

# Update and/or initialize submodules
git submodule update --init

if ! command -v bazel; then
	echo "Please install bazel first: https://github.com/bazelbuild/bazelisk/releases/latest"
fi

pushd third_party

__libusb() {
	pushd libusb

	# Run GNU autoconf
	./bootstrap.sh
	
	# -fPIC: Position Independent Code (tells the linker to not use specific locations)
	# --enable-{shared,static}: Enables building the library's statically- and dynamically-linked versions
	# --disable-udev: 
	CFLAGS="-fPIC" ./configure --enable-static --enable-shared --disable-udev --prefix="$(pwd)/build"

	make
	make install

	# Set the pkgconfig search path
	# pkgconfig is a common utility for finding and configuring libraries to link to on Linux
	export PKG_CONFIG_PATH="$(pwd)/build/lib/pkgconfig"
	
	popd #libusb
}
__libusb

__libedgetpu() {
	pushd libedgetpu

	# Build it
	make libedgetpu

	pushd out
	mv direct/*/libedgetpu.so.1.0 direct/libedgetpu.so
	mv throttled/*/libedgetpu.so.1.0 throttled/libedgetpu.so
	popd #out

	popd #libedgetpu
}
__libedgetpu

__tflite() {
	pushd tensorflow
	mkdir -p build
	pushd build

	cmake ../tensorflow/lite/c/
	make

	popd #build
	popd #tensorflow
}
__tflite

popd #third_party
