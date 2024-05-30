#!/bin/bash

# This script goes through the process of bootstrapping the build junk, then building third-party C++ libs for machine learning.
#
# I tried to comment things decent, but Linux experience is recommended.
# This works with yash or similar, but not with minimal shells like dash.
# pushd/popd is nice.

# Update and/or initialize submodules
git submodule update --init

# If Bazel isn't installed, go through the entire process to download/bootstrap it w/ Bazelisk :/
if ! command -v bazel; then
	#echo "Please install bazel first: https://github.com/bazelbuild/bazelisk/releases/latest"

	case "$(uname -m)" in
		x86_64)
			bazelisk_arch='amd64'
			;;
		arm64)
			bazelisk_arch='arm64'
			;;
	esac

	wget -O bazel https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-${bazelisk_arch}

	if test "$(whoami)" = root; then
		chmod +x ./bazel
		mv ./bazel /usr/local/bin
	else 
		if command -v sudo >/dev/null; then
			sudo chmod +x ./bazel
			sudo mv ./bazel /usr/local/bin
		else
			if command -v doas >/dev/null; then
				doas chmod +x ./bazel
				doas mv ./bazel /usr/local/bin
			else
				echo 'Failed to elevate privileges'
				exit 1
			fi
		fi
	fi
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
