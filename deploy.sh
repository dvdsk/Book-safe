#!/usr/bin/env bash
set -e

# Book locker compile-deploy script, needs: 
#  - rust build system [cargo] (https://www.rust-lang.org/tools/install)
#  - rust cross compile tool [cross] (cargo install cargo-cross)
# please set the variables directly below

SERVER_ADDR="remarkable"
SERVER_USER="root"
SERVER_DIR="/home/$SERVER_USER/book-safe"

dir=debug
if [ "$1" = "--release" ]; then
	dir=release
fi

cross build --target=armv7-unknown-linux-gnueabihf $1
rsync -vh --progress \
  target/armv7-unknown-linux-gnueabihf/$dir/book-safe \
  $SERVER_ADDR:/tmp/

cmds="
mkdir -p $SERVER_DIR
mv /tmp/book-safe $SERVER_DIR/book-safe
chown $SERVER_USER:$SERVER_USER $SERVER_DIR/book-safe
"

ssh -t $SERVER_ADDR "$cmds"
