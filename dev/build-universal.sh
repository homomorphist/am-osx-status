#!/usr/bin/env bash
# Thanks: https://kushshukla24.github.io/blog/builds/mac/rust-universal/
set -euo pipefail
set -o errexit

trap 'kill 0; exit 1' INT

cd "$(dirname "$0")/.."

mkdir -p "./target/universal"
./dev/generate-info-plist.sh "./target/universal"

export RUSTFLAGS="-C link-arg=-sectcreate \
    -C link-arg=__TEXT \
    -C link-arg=__info_plist \
    -C link-arg=$(pwd)/target/universal/Info.plist
    --remap-path-prefix=$(pwd)=."

rustup target add aarch64-apple-darwin &
rustup target add x86_64-apple-darwin &
wait
if [ $? -ne 0 ]; then
    echo "Failed to add targets!"
    exit 1
fi

echo "Targets are installed!"

TARGET_HOST=$(rustc -vV | sed -n 's|host: ||p')
TARGET_OTHER=$(if [ "$TARGET_HOST" = "x86_64-apple-darwin" ]; then echo "aarch64"; else echo "x86_64"; fi)-apple-darwin

echo "${TARGET_HOST} is host; also building for ${TARGET_OTHER}"
echo "Building..."

cargo build --release &
cargo build --release --target ${TARGET_OTHER} &
wait
if [ $? -ne 0 ]; then
    echo "Failed to build!"
    exit 1
fi

if [ -x "$(command -v upx)" ]; then
    echo "Compressing..."
    upx --best --lzma ./target/release/am-osx-status &
    upx --best --lzma ./target/${TARGET_OTHER}/release/am-osx-status &
    wait
    if [ $? -ne 0 ]; then
        echo "Failed to upx compress!"
        exit 1
    fi
fi

echo "Merging..."
lipo -create -output ./target/universal/am-osx-status\
    ./target/release/am-osx-status \
    ./target/${TARGET_OTHER}/release/am-osx-status

otool -L target/universal/am-osx-status | grep -vE "^\s*(/usr/lib|/System/Library|target/universal)" && {
    echo "saw non-system library dependencies, please rewrite this script to fix it by looking at the credited blogpost :^)"
    echo "aborting..."
    exit 1
}

echo "Signing..."
codesign -s - --timestamp=none \
    --identifier network.goop.am-osx-status \
    --options=runtime \
    target/universal/am-osx-status

echo "Done! ./target/universal/am-osx-status"
