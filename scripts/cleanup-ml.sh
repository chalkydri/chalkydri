#!/bin/bash

pushd third_party

if ! command -v bazel; then
	echo "Please install bazel first: https://github.com/bazelbuild/bazelisk/releases/latest"
	return 1
fi

__libusb() {
	pushd libusb
	
	make clean
	
	popd #libusb
}
__libusb

__libedgetpu() {
	pushd libedgetpu

	bazel clean
	make clean

	popd #libedgetpu
}
__libedgetpu

__tflite() {
	pushd tensorflow
	pushd build

	make clean

	popd #build
	popd #tensorflow
}
__tflite

popd #third_party
