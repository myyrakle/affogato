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

## Serving (Linux, systemd)

first, move binary to bin directory
```bash
sudo mv $HOME/.cargo/bin/affogato /usr/bin/affogato
```

second, Second, write systemd config. [reference](./affogato.service)
```bash
sudo vi /etc/systemd/system/affogato.service
```

finally, Activate the daemon using systemctl.
```bash
sudo systemctl enable affogato
sudo systemctl start affogato
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