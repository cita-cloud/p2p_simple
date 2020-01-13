# p2p_simple

A wrapper about [tentacle](https://crates.io/crates/tentacle) to make things simple.

Enable Secio as default. 

No p2p discovery.

### example

There is an example in `examples/test.rs`.

Run it:

```
RUST_LOG=info cargo run --example test 0
RUST_LOG=info cargo run --example test 1
```

