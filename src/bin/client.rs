use std::net::SocketAddr;

use anyhow::{Context, Result};
use remarkable_events::ToolEvent;
use tokio::{io::AsyncReadExt, net};

use mouse_keyboard_input::VirtualDevice;

const BUF_SIZE: usize = 1024;

#[derive(Debug)]
struct P {
    x: f32,
    y: f32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let target_addr = std::env::args()
        .nth(1)
        .context("First arg should be target ip")?
        .parse::<SocketAddr>()
        .context("First arg should be a valid SocketAddr")?;

    println!("Connecting to `{target_addr}`");

    let mut device = VirtualDevice::new();

    let sock = net::TcpSocket::new_v4()?;

    let mut stream = sock.connect(target_addr).await?;

    let mut buf = [0u8; BUF_SIZE];

    let mut latest_point = Option::<P>::None;

    loop {
        let header = stream.read_u64().await? as usize;

        if BUF_SIZE < header {
            eprintln!("Header exceeded buffer size ({BUF_SIZE}): {header}. Stopping");
            break;
        }

        stream.read_exact(&mut buf[0..header]).await?;

        const RM_MAX_Y: f32 = 21000.;
        const RM_MAX_X: f32 = 15725.;
        const SCALE: f32 = 0.2;

        let tool_event = bincode::deserialize::<ToolEvent>(&buf[0..header])?;

        match tool_event {
            ToolEvent::Update(tool) => {
                let new_p = P {
                    x: tool.point.x as f32 / RM_MAX_X,
                    y: tool.point.y as f32 / RM_MAX_Y,
                };

                println!("GOT {new_p:?}");

                if let Some(prev_point) = latest_point {
                    let dx = new_p.x - prev_point.x;
                    let dy = new_p.y - prev_point.y;
                    println!("deltas: {dx},{dy}");

                    let mx = (RM_MAX_X * dx * SCALE) as i32;
                    let my = (RM_MAX_Y * dy * SCALE) as i32;

                    println!("moving {mx},{my}");
                    device.move_mouse(mx, -my).context("moving mouse")?;
                }

                latest_point = Some(new_p);
            }

            ToolEvent::Removed => {
                latest_point = None;
            }
        }
    }

    Ok(())
}
