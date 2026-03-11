#!/bin/bash

sudo nmtui
sudo bash -c 'echo "nameserver 1.1.1.1" > /etc/resolv.conf'
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > install.sh
chmod +x install.sh
./install.sh --default-toolchain none
. "$HOME/.cargo/env"
rm ./install.sh
sudo apt update
sudo apt upgrade
sudo apt install gcc
sudo apt install build-essential
#sudo ip link set eth0 up
sudo apt install pkg-config libssl-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
sudo apt install cmake
sudo apt install gstreamer1.0-tools
sudo apt install gstreamer1.0-plugins-good
sudo apt install gstreamer1.0-plugins-bad
sudo apt install gstreamer1.0-vaapi
sudo apt install v4l-utils
sudo apt install gstreamer1.0-libcamera gstreamer1.0-plugins-base
sudo apt install pipewire
sudo apt install pipewire-v4l2
sudo apt install btop
sudo systemctl start udevd
cargo b -r -p chalkydri_configurator
cargo b -r -p chalkydri --bin chalkydri
git clone -b ubuntu_setup --single-branch https://github.com/rubikpi-ai/rubikpi-script.git
cd rubikpi-script
./install_ppa_pkgs.sh
cd ..
rm -r rubikpi-script
sudo apt dist-upgrade
sudo reboot now
