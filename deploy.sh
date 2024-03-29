#!/usr/bin/env bash
set -e

# Book locker compile-deploy script, needs: 
#  - rust build system [cargo] (https://www.rust-lang.org/tools/install)
#  - rust cross compile tool [cross] (cargo install cargo-cross)
# please set the variables directly below

SERVER_ADDR="remarkable"
SERVER_DIR="/home/root"

cross build --target=armv7-unknown-linux-gnueabihf --release
rsync -vh --progress \
  target/armv7-unknown-linux-gnueabihf/release/book-safe \
  $SERVER_ADDR:/tmp/

cmds="
mv /tmp/book-safe $SERVER_DIR/book-safe
chown root:root $SERVER_DIR/book-safe
"

ssh -t $SERVER_ADDR "$cmds"
