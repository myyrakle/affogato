# affogato

![](https://img.shields.io/badge/language-Rust-red) ![](https://img.shields.io/badge/version-0.1.0-brightgreen) [![GitHub license](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/myyrakle/affogato/blob/master/LICENSE)

simple HTTP/HTTPS Proxy

## Install 

```bash
cargo install affogato
```

## Run 

```bash 
affogato -p 3000
```

## Zero-Downtime Reload (Linux Only)

first server 
```
cargo run 
```

deploy new server without downtime
```
pkill -SIGQUIT affogato && cargo run -- -u
```