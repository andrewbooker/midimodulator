#!/bin/bash

die() {
    echo $1
    exit 1
}

assert_success() {
    if [[ -n $1 ]]
    then
        echo "can $2"
    else
        die "!! cannot $2 !!"
    fi
}

assert_success "$(lsusb | grep '4i4o')" "see 4-way USB Midi box"
assert_success "$(lsusb | grep 'Serial Port')" "see at least one USB serial port"
echo 'all good'
