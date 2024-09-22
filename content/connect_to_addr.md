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
        //: non-blocking syscall connect
        let sys = mio::net::TcpStream::connect(addr)?;
        TcpStream::connect_mio(sys).await
    }
}
```

## connect_mio

```rust
impl TcpStream {
    async fn connect_mio(sys: mio::net::TcpStream) -> io::Result<TcpStream> {
        //: wraps mio TcpStream(OS non-blocking socket) with a event-notification reactor
        let stream = TcpStream::new(sys)?;

        //: wraps a function returning Poll<T> into a Future
        //: wait the Future to complete: connected or failure
        poll_fn(|cx| stream.io.registration().poll_write_ready(cx)).await?;

        if let Some(e) = stream.io.take_error()? {
            return Err(e);
        }

        Ok(stream)
    }
}
```

## io-event reactor

```rust
//: tokio::net::tcp::TcpStream
struct TcpStream {
    io: PollEvented<mio::net::TcpStream>,
}

struct PollEvented<E: Source> {
    io: Option<E>,
    registration: Registration,
}

struct Registration {
    handle: scheduler::Handle,

    /// Reference to state stored by the driver.
    shared: Arc<ScheduledIo>,
}

#[repr(align(128))]
struct ScheduledIo {
    pub(super) linked_list_pointers: UnsafeCell<linked_list::Pointers<Self>>,

    /// Packs the resource's readiness and I/O driver latest tick.
    readiness: AtomicUsize,

    waiters: Mutex<Waiters>,
}
```

how Registration constructed?

```rust
impl Registration {
    fn new_with_interest_and_handle(
        io: &mut impl Source,
        interest: Interest,
        handle: scheduler::Handle,
    ) -> io::Result<Registration> {
        //: scheduler::Handle
        //: driver::Handle
        //: IoHandle
        let shared = handle.driver().io().add_source(io, interest)?;

        Ok(Registration { handle, shared })
    }
}

enum IoHandle {
    Enabled(crate::runtime::io::Handle),
    Disabled(UnparkThread),
}

//: crate::runtime::io::Handle
/// A reference to an I/O driver.
pub(crate) struct Handle {
    /// Registers I/O resources.
    registry: mio::Registry,

    /// Tracks all registrations
    registrations: RegistrationSet,

    /// State that should be synchronized
    synced: Mutex<registration_set::Synced>,

    /// Used to wake up the reactor from a call to `turn`.
    /// Not supported on `Wasi` due to lack of threading support.
    #[cfg(not(target_os = "wasi"))]
    waker: mio::Waker,

    pub(crate) metrics: IoDriverMetrics,
}

struct RegistrationSet {
    num_pending_release: AtomicUsize,
}

struct Synced {
    is_shutdown: bool,
    registrations: LinkedList<Arc<ScheduledIo>, ScheduledIo>,
    pending_release: Vec<Arc<ScheduledIo>>,
}

//: crate::runtime::io::Handle
impl Handle {
    fn add_source(
        &self,
        source: &mut impl mio::event::Source,
        interest: Interest,
    ) -> io::Result<Arc<ScheduledIo>> {
        let scheduled_io = self.registrations.allocate(&mut self.synced.lock())?;
        let token = scheduled_io.token();

        //: mio::Registry
        self.registry.register(source, token, interest.to_mio());

        self.metrics.incr_fd_count();

        Ok(scheduled_io)
    }
}

fn allocate(&self, synced: &mut Synced) -> io::Result<Arc<ScheduledIo>> {
    if synced.is_shutdown {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            crate::util::error::RUNTIME_SHUTTING_DOWN_ERROR,
        ));
    }

    let ret = Arc::new(ScheduledIo::default());

    // Push a ref into the list of all resources.
    synced.registrations.push_front(ret.clone());

    Ok(ret)
}
```

Make use of `mio::Registry` to register a writable event. Wait for OS IO event
notification.

This `mio::Registry` is encapsulated in `crate::runtime::io::Handle`.
When does this io::Handle get created?
In [[hello_world##build_threaded_runtime]]
