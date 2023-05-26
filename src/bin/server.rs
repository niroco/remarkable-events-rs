use anyhow::Result;
use bytes::{BufMut, BytesMut};
use remarkable_events::ToolEventSource;
use std::net::SocketAddr;

use tokio::{
    io::AsyncWriteExt,
    net::{self, TcpStream},
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // let target = std::env::args().nth(1).unwrap_or_else(|| String::from("/dev/input/event1"));

    let socket_addr = "0.0.0.0:6281".parse::<SocketAddr>()?;

    let mut counter = 0;
    let listener = net::TcpListener::bind(socket_addr).await?;
    println!("Listening on {socket_addr}");

    loop {
        counter += 1;
        let (socket, addr) = listener.accept().await?;

        let session = Session {
            nr: counter,
            socket,
            addr,
        };

        tokio::spawn(async move {
            if let Err(err) = session.run().await {
                eprintln!("Stopping session with err: {err}");
            }
        });
    }
}

struct Session {
    nr: usize,
    socket: TcpStream,
    addr: SocketAddr,
}

impl Session {
    pub async fn run(mut self) -> Result<()> {
        let mut tool_events = ToolEventSource::open("/dev/input/event1").await?;

        let mut buf = BytesMut::new().writer();
        println!("Starting Session #{} from {}", self.nr, self.addr);

        loop {
            match tool_events.next().await {
                Ok(ev) => {
                    bincode::serialize_into(&mut buf, &ev)?;
                    let l = buf.get_ref().len();
                    println!("Serialized to {l} bs");

                    self.socket.write_u64(l as u64).await?;
                    println!("Wrote header");

                    self.socket.write_all(&buf.get_ref()[0..l]).await?;

                    println!("Wrote body");
                    buf.get_mut().clear();
                }

                Err(err) => {
                    eprintln!("error: {err}");
                    break;
                }
            }
        }

        Ok(())
    }
}
