# brainfuck

A simple brainfuck interpreter, with JIT support.

## Prerequisites

### macOS
You need to install `llvm@18` and `zstd` using Homebrew:
```shell
brew install llvm@18 zstd
```

### Linux
You need to install `llvm-18-dev` and `libpolly-18-dev`:
```shell
sudo apt install llvm-18-dev libpolly-18-dev
```

## Build

```shell
cargo build --release
```

## Run

```shell
./target/release/bf <path-to-bf-file>
```

Or run with cranelift-jit/llvm-jit:

```shell
./target/release/bf <path-to-bf-file> jit --method [cranelift | llvm]
```

If you want to dump the ir:

```shell
./target/release/bf <path-to-bf-file> jit --method [cranelift | llvm] --dump-ir
```

## FAQ

### Build with LLVM Support

If you encounter issues about missing `LLVM_SYS_xxx_PREFIX`, you can build with the following command:

#### macOS
```shell
LLVM_SYS_180_PREFIX="/opt/homebrew/opt/llvm@18" cargo build --release
```

#### Ubuntu
```shell
LLVM_SYS_180_PREFIX="/usr/lib/llvm-18" cargo build --release
```

### Customizing LLVM Environment Variables

If you need to modify LLVM-related environment variables, refer to `.cargo/config.toml`.

## LICENSE

This project is licensed under [the MIT License](./LICENSE).

## Acknowledgements

* [bf-jit](https://github.com/QRWells/bf-jit)
