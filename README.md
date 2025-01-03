# brainfuck

A simple brainfuck interpreter, with JIT support.

## Build

```shell
cargo build -- release
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

## LICENSE

This project is licensed under [the MIT License](./LICENSE).

## Acknowledgements

* [bf-jit](https://github.com/QRWells/bf-jit)
