use std::net::SocketAddr;

use anyhow::{Context, Result};
use remarkable_events::ToolEvent;
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

    let mm = mouce::nix::NixMouseManager::new();

    let (x, y) = mm.get_position()?;
    println!("mouse at {x},{y}");

    let mut stream = sock.connect(target_addr).await?;

    let mut buf = [0u8; BUF_SIZE];

    loop {
        let header = stream.read_u64().await? as usize;

        if BUF_SIZE < header {
            eprintln!("Header exceeded buffer size ({BUF_SIZE}): {header}. Stopping");
            break;
        }

        stream.read_exact(&mut buf[0..header]).await?;

        const RM_MAX_Y: u32 = 21000;
        const RM_MAX_X: u32 = 15725;

        let tool_event = bincode::deserialize::<ToolEvent>(&buf[0..header])?;

        match tool_event {
            ToolEvent::Update(tool) => {
                //let x = tool.point.x / 10;
                //let y = (1440 - (tool.point.y / 10));

                let x = tool.point.x as f32 / RM_MAX_X as f32;
                let y = tool.point.y as f32 / RM_MAX_Y as f32;

                println!("{x}, {y}");

                //mm.move_to(x as usize, y as usize)?;
            }

            other => {
                println!("Got event: {other:?}");
            }
        }
    }

    Ok(())
}
