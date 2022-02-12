#!/usr/bin/env bash
set -e

# Book locker compile-deploy script, needs: 
#  - rust build system [cargo] (https://www.rust-lang.org/tools/install)
#  - rust cross compile tool [cross] (cargo install cargo-cross)
# please set the variables directly below

SERVER_ADDR="remarkable"
SERVER_USER="root"
SERVER_DIR="/home/$SERVER_USER/"

dir=debug
if [ "$1" = "--release" ]; then
	dir=release
fi

cross build --target=armv7-unknown-linux-gnueabihf $1
# rsync llama_lamps.service $SERVER_ADDR:/tmp/
rsync -vh --progress \
  target/armv7-unknown-linux-gnueabihf/$dir/book-lock \
  $SERVER_ADDR:/tmp/

# # sets up/updates the systemd service and places the binary
# cmds="
# sed -i \"s/<USER>/$SERVER_USER/g\" /tmp/llama_lamps.service
# sed -i \"s+<DIR>+$SERVER_DIR+g\" /tmp/llama_lamps.service
# sudo mv /tmp/llama_lamps.service /etc/systemd/system/
# mkdir -p $SERVER_DIR
# mv /tmp/llama_lamps $SERVER_DIR/llama_lamps
# chown $SERVER_USER:$SERVER_USER $SERVER_DIR/llama_lamps
# sudo systemctl enable llama_lamps.service
# sudo systemctl restart llama_lamps.service
# "

# ssh -t $SERVER_ADDR "$cmds"
