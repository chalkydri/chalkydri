#!/bin/bash

# This script goes through the process of bootstrapping the build junk, then building third-party C++ libs for machine learning.
#
# I tried to comment things decent, but Linux experience is recommended.
# This works with yash or similar, but not with minimal shells like dash.
# pushd/popd is nice.

# I have to use three different joke build systems to use Google's trash.

flatbuffers_version='v23.5.26'
libusb_version='v1.0.27'
libedgetpu_version='v0.1.9'
tensorflow_version='v2.16.1'

install_prefix='/build/install-prefix'

# # If Bazel isn't installed, go through the entire process to download/bootstrap it w/ Bazelisk :/
# if ! command -v bazel; then
# 	#echo "Please install bazel first: https://github.com/bazelbuild/bazelisk/releases/latest"

# 	# Pick the correct CPU architecture for the current system
# 	case "$(uname -m)" in
# 		x86_64)
# 			bazelisk_arch='amd64'
# 			;;
# 		arm64)
# 			bazelisk_arch='arm64'
# 			;;
# 	esac

# 	# Download bazelisk
# 	wget -O bazel https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-${bazelisk_arch}

# 	# Try to get root somehow and install it
# 	if test "$(whoami)" = root; then
# 		chmod +x ./bazel
# 		mv ./bazel /usr/local/bin
# 	else 
# 		if command -v sudo >/dev/null; then
# 			sudo chmod +x ./bazel
# 			sudo mv ./bazel /usr/local/bin
# 		else
# 			if command -v doas >/dev/null; then
# 				doas chmod +x ./bazel
# 				doas mv ./bazel /usr/local/bin
# 			else
# 				echo 'Failed to elevate privileges'
# 				exit 1
# 			fi
# 		fi
# 	fi
# fi

__flatbuffers() {
	pushd flatbuffers
 	git checkout $flatbuffers_version
	mkdir -p build
 	pushd build
  
	cmake -DFLATBUFFERS_BUILD_SHAREDLIB=ON -DFLATBUFFERS_BUILD_TESTS=OFF -DCMAKE_BUILD_TYPE=Release -DCMAKE_STATIC_LIBRARY=TRUE -DCMAKE_INSTALL_PREFIX=/usr ..
	make
	make install

 	popd #build
  	popd #flatbuffers
}
__flatbuffers

__tflite() {
	pushd tensorflow
	git checkout $tensorflow_version
	mkdir -p build
	pushd build

	cmake -DCMAKE_SHARED_LIBRARY=TRUE -DCMAKE_STATIC_LIBRARY=TRUE -DCMAKE_BUILD_TYPE=Release ../tensorflow/lite/c/
	make

	popd #build

	mkdir -p $install_prefix/include $install_prefix/lib
 	find . -name 'tensorflow/lite/*.h' -exec cp --parents '{}' $install_prefix/include \;
  	find . -name 'build/*.a' -exec cp '{}' $install_prefix/lib \;
  	find . -name 'build/*.a' -exec cp '{}' /usr/local/lib \;
   	find . -name 'build/*.so' -exec cp '{}' $install_prefix/lib \;
   	find . -name 'build/*.so' -exec cp '{}' /usr/local/lib \;
  
	popd #tensorflow
}
__tflite

__libusb() {
	pushd libusb
	git checkout $libusb_version

	# Run GNU autoconf
	./bootstrap.sh
	
	# -fPIC: Position Independent Code (tells the linker to not use specific locations)
	# --enable-{shared,static}: Enables building the library's statically- and dynamically-linked versions
	# --disable-udev: 
	CFLAGS="-fPIC" ./configure --enable-static --enable-shared --disable-udev --prefix="/build/install-prefix"

	make
	make install

	# Set the pkgconfig search path
	# pkgconfig is a common utility for finding and configuring libraries to link to on Linux
	#export PKG_CONFIG_PATH="$install_prefix/lib/pkgconfig"
	
	popd #libusb
}
__libusb

__libedgetpu() {
	pushd libedgetpu
	git checkout $libedgetpu_version

	# Build it
	export CFLAGS="-L/build/install-prefix/lib -I/build/install-prefix/include"
	export CXXFLAGS="-L/build/install-prefix/lib -I/build/install-prefix/include"
 	export LDFLAGS="-L/build/install-prefix/lib"
  	TFROOT=/build/tensorflow/ LD_LIBRARY_PATH=/build/install-prefix/lib/ make -f makefile_build/Makefile libedgetpu

	pushd out
	mv direct/*/libedgetpu.so.1.0 direct/libedgetpu.so
	mv throttled/*/libedgetpu.so.1.0 throttled/libedgetpu.so
	popd #out

	popd #libedgetpu
}
__libedgetpu
