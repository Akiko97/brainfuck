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

Or run with cranelift-jit:

```shell
./target/release/bf <path-to-bf-file> jit --method cranelift
```

If you want to dump the cranelift-ir:

```shell
./target/release/bf <path-to-bf-file> jit --method cranelift --dump-ir
```

## LICENSE

This project is licensed under [the MIT License](./LICENSE).

## Acknowledgements

* [bf-jit](https://github.com/QRWells/bf-jit)
