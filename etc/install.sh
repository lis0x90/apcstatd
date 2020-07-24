#!/bin/bash

set -ex

cd ..
cargo build --release 
sudo cp -f target/release/apcstatd /bin/apcstatd

cd etc
sudo cp -f apcstatd.service /lib/systemd/system/
sudo chmod 644 /lib/systemd/system/apcstatd.service
sudo systemctl daemon-reload
sudo systemctl stop apcstatd.service || true
sudo systemctl disable apcstatd.service || true
sudo systemctl enable apcstatd.service
sudo systemctl start apcstatd.service