#!/bin/bash


#ARCH="armv7-unknown-linux-musleabihf"
TARGET="root@192.168.1.192"
ARCH="armv7-unknown-linux-gnueabihf"

echo "Compiling..."
cross build --target "$ARCH" --release --example print_events || exit 1
echo "Done"

echo "Copying target/$ARCH/release/examples/print_events to $TARGET:~/"
scp target/$ARCH/release/examples/print_events $TARGET:~/
echo "Done"

echo "Making binary executable"
ssh $TARGET "chmod +x remlabs"
echo "Done"
