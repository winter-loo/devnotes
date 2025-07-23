---
id: building
aliases: []
tags: []
---

## build for development

```bash
git submodule update
cd sqlite
mkdir build && cd build
CFLAGS="-g" ../configure --enable-debug
make -j
```


## LSP configuration

[.clangd configuration file](https://clangd.llvm.org/config.html#compileflags) for neovim + clangd:

```.clangd
CompileFlags:
  Add: 
    - -I /home/ldd/notes-sqlite/sqlite/build
```
