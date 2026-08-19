if [ -z $(which rustup) ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
fi
sudo apt install -y libssl-dev libportmidi-dev 


cd ~/Downloads
wget http://archive.ubuntu.com/ubuntu/pool/universe/r/rtmidi/librtmidi-dev_3.0.0~ds1-2build1_amd64.deb
sudo dpkg -i librtmidi-dev_3.0.0~ds1-2build1_x86.deb
cd -
sudo apt install -y clang
sudo apt install -y libudev-dev
