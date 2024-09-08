## demo

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

## TcpStream::connect

```rust
pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
    let addrs = to_socket_addrs(addr).await?;
    // ...
}

pub(crate) fn to_socket_addrs<T>(arg: T) -> T::Future
    where
        T: ToSocketAddrs,
    {
        arg.to_socket_addrs(sealed::Internal)
    }

//! interface/implementation pattern, wrapper, ecapsulation
pub trait ToSocketAddrs: sealed::ToSocketAddrsPriv {}

mod sealed {
    #[doc(hidden)]
    pub trait ToSocketAddrsPriv {
        type Iter: Iterator<Item = SocketAddr> + Send + 'static;
        type Future: Future<Output = io::Result<Self::Iter>> + Send + 'static;

        fn to_socket_addrs(&self, internal: Internal) -> Self::Future;
    }
}

impl ToSocketAddrs for str {}

//! net/addr.rs
impl sealed::ToSocketAddrsPriv for str {
    type Iter = sealed::OneOrMore;
    type Future = sealed::MaybeReady;

    fn to_socket_addrs(&self, _: sealed::Internal) -> Self::Future {
        use crate::blocking::spawn_blocking;
        use sealed::MaybeReady;

        // First check if the input parses as a socket address
        //! https://doc.rust-lang.org/stable/std/net/enum.SocketAddr.html
        let res: Result<SocketAddr, _> = self.parse();

        if let Ok(addr) = res {
            return MaybeReady(sealed::State::Ready(Some(addr)));
        }

        // Run DNS lookup on the blocking pool
        //! convert `str` to a heap-allocated String
        let s = self.to_owned();

        //! - `spawn_blocking` noted in later section.
        //! - `MaybeReady` is a manually-implemented Future
        MaybeReady(sealed::State::Blocking(spawn_blocking(move || {
            std::net::ToSocketAddrs::to_socket_addrs(&s)
        })))
    }
}
```

## MaybeReady

TODO: net/addr.rs:182 MaybeReady implementation

## spawn_blocking

TODO: net/addr.rs:182 spawn_blocking internals
