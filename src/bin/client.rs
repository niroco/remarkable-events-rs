use std::net::SocketAddr;

use anyhow::{Context, Result};
use bytes::BytesMut;
use remarkable_events::{Tool, ToolEvent};
use tokio::{io::AsyncReadExt, net};

const BUF_SIZE: usize = 1024;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let target_addr = std::env::args()
        .nth(1)
        .context("First arg should be target ip")?
        .parse::<SocketAddr>()
        .context("First arg should be a valid SocketAddr")?;

    println!("Connecting to `{target_addr}`");

    let sock = net::TcpSocket::new_v4()?;

    //let mut device = VirtualDevice::new();
    let mut stream = sock.connect(target_addr).await?;

    let mut buf = [0u8; BUF_SIZE];

    loop {
        let header = stream.read_u64().await? as usize;

        if BUF_SIZE < header {
            eprintln!("Header exceeded buffer size ({BUF_SIZE}): {header}. Stopping");
            break;
        }

        println!("Read header `{header}`");

        stream.read_exact(&mut buf[0..header]).await?;

        println!("Read body");

        let tool_event = bincode::deserialize::<ToolEvent>(&buf[0..header])?;

        match tool_event {
            ToolEvent::Update(tool) => {
                println!("{tool:?}");
            }

            other => {
                println!("Got event: {other:?}");
            }
        }
    }

    Ok(())
}
