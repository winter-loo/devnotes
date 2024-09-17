# demo

```rust
use tokio::net::TcpStream;

use std::error::Error;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // Open a TCP stream to the socket address.
    //
    // Note that this is the Tokio TcpStream, which is fully async.
    let mut stream = TcpStream::connect("127.0.0.1:6142").await?;
    println!("created stream");

    Ok(())
}
```

# internals

[[get_socket_address|part 1: Get Socket Address]]

[[connect_to_addr|part 2: connect to an address]]
