# affogato

HTTP Proxy

## Zero-Downtime Reload (Linux Only)

first server 
```
cargo run 
```

deploy new server without downtime
```
pkill -SIGQUIT affogato && cargo run -- -u
```