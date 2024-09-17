![[tcp_connect#demo]]

# Connect To An Address

```rust
impl TcpStream {
    async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
        let addrs = to_socket_addrs(addr).await?;

        let mut last_err = None;

        for addr in addrs {
            match TcpStream::connect_addr(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => last_err = Some(e),
            }
        }
        //: handle last_err
    }

    async fn connect_addr(addr: SocketAddr) -> io::Result<TcpStream> {
        let sys = mio::net::TcpStream::connect(addr)?;
        TcpStream::connect_mio(sys).await
    }
}
```

## connect_mio

```rust
impl TcpStream {
    async fn connect_mio(sys: mio::net::TcpStream) -> io::Result<TcpStream> {
        let stream = TcpStream::new(sys)?;

        poll_fn(|cx| stream.io.registration().poll_write_ready(cx)).await?;

        if let Some(e) = stream.io.take_error()? {
            return Err(e);
        }

        Ok(stream)
    }
}
```

TODO:
 - mio
 - poll_fn
