#!/bin/bash


#ARCH="armv7-unknown-linux-musleabihf"
TARGET="root@192.168.1.56"
ARCH="armv7-unknown-linux-gnueabihf"

echo "Compiling..."
cross build --target "$ARCH" --release --bin $1 || exit 1
echo "Done"

echo "Copying target/$ARCH/release/$1 to $TARGET:~/"
scp target/$ARCH/release/$1 $TARGET:~/
echo "Done"

#echo "Making binary executable"
#ssh $TARGET "chmod +x remlabs"
#echo "Done"
