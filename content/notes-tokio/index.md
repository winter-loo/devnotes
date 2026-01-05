# tokio-notes

Code Notes About [Tokio Source Code](https://github.com/tokio-rs/tokio/tree/1166ecc2accc1a4bab47612858e7166617d15cfe)


Time flys to 2025 and AI is everywhere. I don't find code path by myself any longer. DeekWiki can answer my questions:

[source code](https://github.com/tokio-rs/tokio/tree/5e3ad02fb106f72194c13bdff8317e9a99953c05) deepwiki learnt

- [how does tokio use mio crate in its core runtime](https://deepwiki.com/search/how-does-tokio-use-mio-crate-i_3d7bbb62-f92f-4a46-ad33-5a1ab952f47c)
- [what's the "let ptr = super::EXPOSE_IO.from_exposed_addr(token.0);" in driver.rs](https://deepwiki.com/search/how-does-tokio-use-mio-crate-i_3d7bbb62-f92f-4a46-ad33-5a1ab952f47c)
- [how's the tokio executor polling futures](https://deepwiki.com/search/how-does-tokio-use-mio-crate-i_3d7bbb62-f92f-4a46-ad33-5a1ab952f47c)
- [Help me understand deeply the design of variant Task struct: RawTask, Task, Notified, LocalNotified, JoinHandle](https://deepwiki.com/search/help-me-understand-deeply-the_d23a0771-d4b4-4cae-9414-bb3d2dd05acd)

* [[hello_world]]
* [[tcp_connect]]
* [[echo]]

## diagrams

![ctrl_ctrl_c_future.png](./image/tokio_ctrl_c_future.png)

## references

- [Making the Tokio scheduler 10x faster](https://tokio.rs/blog/2019-10-scheduler)
