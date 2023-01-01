# wgpu-template

Running:

```bash
# regular
cargo run --release

# wasm/web
RUSTFLAGS="--cfg=web_sys_unstable_apis" wasm-pack build --release --target web
serve . # open localhost
```
