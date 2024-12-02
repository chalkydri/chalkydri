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

install_prefix='/deps'

export PKG_CONFIG_PATH="/deps/lib/pkgconfig"

__flatbuffers() {
	pushd flatbuffers
 	git checkout $flatbuffers_version
	mkdir -p build
 	pushd build
  
	cmake -DFLATBUFFERS_BUILD_SHAREDLIB=OFF -DFLATBUFFERS_BUILD_TESTS=OFF -DCMAKE_BUILD_TYPE=Release -DFLATBUFFERS_BUILD_STATICLIB=TRUE -DCMAKE_INSTALL_PREFIX=/deps ..
	cmake --build . -j
	cmake --install .

 	popd #build
  	popd #flatbuffers
}
__flatbuffers

__tflite() {
	pushd tensorflow
	#git checkout $tensorflow_version
	mkdir -p build
	pushd build

	cmake -DTFLITE_ENABLE_XNNPACK=OFF -DCMAKE_BUILD_TYPE=Release -DCMAKE_LIBRARY_PATH=/deps/lib \
 		-DCMAKE_SHARED_LINKER_FLAGS="-lbsd" -DTFLITE_C_BUILD_SHARED_LIBS=OFF \
 		../tensorflow/lite/c/
	cmake --build . -j

	# popd #build

	mkdir -p /deps/include /deps/lib
 	find tensorflow/lite -type f -name '*.h' -exec cp --parents '{}' /usr/local/include \;
   	find tensorflow/lite -type f -name '*.h' -exec cp --parents '{}' /deps/include \;
  	# find build -type f -name '*.a' -exec cp '{}' /deps/lib \;
   # 	find build -type f -name '*.so' -exec cp '{}' /deps/lib \;
  
	popd #tensorflow
}
__tflite

# __libusb() {
# 	pushd libusb
# 	git checkout $libusb_version

# 	# Run GNU autoconf
# 	./bootstrap.sh
	
# 	# -fPIC: Position Independent Code (tells the linker to not use specific locations)
# 	# --enable-{shared,static}: Enables building the library's statically- and dynamically-linked versions
# 	# --disable-udev: 
# 	CFLAGS="-fPIC" ./configure --enable-static --enable-shared --disable-udev --prefix="/deps"

# 	make
# 	make install
	
# 	popd #libusb
# }
# __libusb

# __abseil_is_bs() {
# 	git clone https://github.com/abseil/abseil-cpp.git
# 	pushd abseil-cpp
# 	mkdir -p build
# 	pushd build

# 	cmake -DCMAKE_SHARED_LIBRARY=TRUE -DCMAKE_STATIC_LIBRARY=TRUE -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="/deps" ../
# 	make
#  	make install

#   	popd #build
#    	popd #abseil-cpp
# }
# __abseil_is_bs

__libedgetpu() {
	pushd libedgetpu
	#git checkout $libedgetpu_version

	# Build it
  	TFROOT=/build/tensorflow/ LD_LIBRARY_PATH=/deps/lib make -f makefile_build/Makefile libedgetpu

	pushd tflite
 	pushd public
  	find . -type f -name '*.h' -exec cp --parents '{}' /deps/include \;
	popd #public
 	popd #tflite
    	
	pushd out
	cp direct/*/libedgetpu.so.1.0 /deps/lib/libedgetpu.so
	popd #out

	popd #libedgetpu
}
__libedgetpu
