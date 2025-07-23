#!/bin/sh -x

# environment setup
# For Cloudfare, set the environment variable NODE_VERSION=22 on its page

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

cp quartz-config/quartz.config.ts quartz-config/quartz.layout.ts quartz/
cp quartz-config/custom.scss quartz/quartz/styles/
cp quartz-config/0xProtoNerdFont-Regular.ttf quartz/quartz/static/
cd quartz && npx quartz build "$@"
