# echo server

## code

[echo.rs](https://github.com/tokio-rs/tokio/blob/1166ecc2accc1a4bab47612858e7166617d15cfe/examples/echo.rs)

```rust
let listener = TcpListener::bind(&addr).await?;
loop {
    let (mut socket, _) = listener.accept().await?;

    tokio::spawn(async move {
        let mut buf = vec![0; 1024];

        // In a loop, read data from the socket and write the data back.
        loop {
            let n = socket
                .read(&mut buf)
                .await
                .expect("failed to read data from socket");

            if n == 0 {
                return;
            }

            socket
                .write_all(&buf[0..n])
                .await
                .expect("failed to write data to socket");
        }
    });
}
```

## internals

[[tokio_spawn]]
