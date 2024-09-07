#!/bin/sh

# environment setup
# Check if OS is Ubuntu and upgrade Node.js to the latest stable version if so
if [ -f /etc/lsb-release ] && grep -q "Ubuntu" /etc/lsb-release; then
    echo "Ubuntu detected. Upgrading Node.js to the latest stable version..."
    sudo apt update
    sudo apt install -y curl
    curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
    sudo apt install -y nodejs
    echo "Node.js version after upgrade: $(node -v)"
fi

if [ "$1" = "-f" ]; then
    echo "Force deletion requested. Removing existing 'quartz' directory..."
    rm -rf quartz
    shift
fi

if [ ! -d "quartz" ]; then
    echo "Directory 'quartz' does not exist. Cloning and setting up..."
    git clone https://github.com/jackyzha0/quartz.git
    cd quartz
    npm i
    npx quartz create -d content -s ../content -X symlink -l shortest
    cd ..
fi

pushd quartz

npx quartz build "$@"
