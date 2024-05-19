#!/bin/sh

pushd deps

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

popd #deps
