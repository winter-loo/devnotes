#!/bin/sh

if [ "$1" = "-f" ]; then
    echo "Force deletion requested. Removing existing 'quartz' directory..."
    rm -rf quartz
fi

if [ ! -d "quartz" ]; then
    echo "Directory 'quartz' does not exist. Cloning and setting up..."
    git clone https://github.com/jackyzha0/quartz.git
    cd quartz
    npm i
    npx quartz create -d content -s ../content -X symlink -l shortest
fi

pushd quartz
npx quartz build --serve
