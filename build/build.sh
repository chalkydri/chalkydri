#!/bin/sh

echo ' >>> Starting...'

if ! command -v apk >/dev/null; then
	echo 'You must be using an Alpine system'
	return 1
fi

echo ' >>> Installing dependencies...'
apk add rustup genimage xorriso squashfs-tools gzip mtools genext2fs e2fsprogs-extra

#rustup-init -y
#source "$HOME/.cargo/env"
#
#echo ' >>> Installing Rust toolchains...'
#rustup target add x86_64-unknown-linux-musl
#rustup target add aarch64-unknown-linux-musl
#
#echo ' >>> Cloning Chalkydri source tree...'
#git clone --recursive https://github.com/chalkydri/chalkydri.git
#
#pushd chalkydri
#
#echo ' >>> Building ML libs...'
#./scripts/build-ml.sh
#
#echo ' >>> Building Chalkydri'
#cargo b -r --target x86_64-unknown-linux-musl
#cargo b -r --target aarch64-unknown-linux-musl
#
#popd

__rpi() {
	#mkdir -p bootfs
	#for pkg in raspberrypi-bootloader-common raspberrypi-bootloader linux-rpi; do
	#	apk fetch --arch aarch64 -X https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/ -U --allow-untrusted --root /dev/null --no-cache --quiet --stdout $pkg | tar -C bootfs -zx
	#done
	#mv bootfs/boot/* bootfs
	#rm bootfs/.*

	#mkdir -p rootfs
	#apk add --arch aarch64 -X https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/ -X https://dl-cdn.alpinelinux.org/alpine/latest-stable/community/ -U --allow-untrusted --root ./rootfs --no-cache --initdb alpine-base raspberrypi-bootloader-common raspberrypi-bootloader linux-rpi libcamera-raspberrypi
	#rm rootfs/.*
	#mv rootfs/boot bootfs

	#genimage --config genimage_rpi.cfg
	
	mkdir -p rootfs
	wget -O - https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/aarch64/alpine-rpi-3.20.0-aarch64.tar.gz | tar -C rootfs -zx
}
__rpi

