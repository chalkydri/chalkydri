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

echo ' >>> Installing Rust toolchains...'
#rustup target add x86_64-unknown-linux-musl
#rustup target add aarch64-unknown-linux-musl

echo ' >>> Building Chalkydri...'
#git clone --recursive https://github.com/chalkydri/chalkydri.git

#pushd chalkydri

#./scripts/build-ml.sh

#popd

__rpi() {
	mkdir -p bootfs
	for pkg in raspberrypi-bootloader raspberrypi-bootloader-common raspberrypi-bootloader-cutdown; do
		apk fetch --arch aarch64 -X https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/ -U --allow-untrusted --root /dev/null --no-cache --quiet --stdout $pkg | tar -C bootfs -zx
	done
	rm bootfs/.*

	mkdir -p rootfs
	apk add --arch aarch64 -X https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/ -X https://dl-cdn.alpinelinux.org/alpine/latest-stable/community/ -U --allow-untrusted --root ./rootfs --no-cache --initdb alpine-base libcamera-raspberrypi
	rm rootfs/.*
}
__rpi

