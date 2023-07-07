use std::{iter::FromIterator, net::SocketAddr};

use anyhow::{Context, Result};
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, Key, RelativeAxisType,
};
use remarkable_events::{Height, ToolEvent};
use tokio::{io::AsyncReadExt, net};

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

    let mut mouse = MouseController::default();

    let sock = net::TcpSocket::new_v4()?;

    let mut stream = sock.connect(target_addr).await?;

    let mut buf = [0u8; BUF_SIZE];

    let mut latest_point = Option::<P>::None;
    let mut dx: f32 = 0.;
    let mut dy: f32 = 0.;
    let mut is_pressed = false;

    loop {
        let header = stream.read_u64().await? as usize;

        if BUF_SIZE < header {
            eprintln!("Header exceeded buffer size ({BUF_SIZE}): {header}. Stopping");
            break;
        }

        stream.read_exact(&mut buf[0..header]).await?;

        const RM_MAX_Y: f32 = 21000.;
        const RM_MAX_X: f32 = 15725.;

        const SCALE: f32 = 0.17;

        let tool_event = bincode::deserialize::<ToolEvent>(&buf[0..header])?;

        match tool_event {
            ToolEvent::Update(tool) => {
                let new_p = P {
                    x: tool.point.x as f32 / RM_MAX_X,
                    y: tool.point.y as f32 / RM_MAX_Y,
                };

                match tool.height {
                    Height::Distance(_) if is_pressed => {
                        mouse.release()?;
                        is_pressed = false;
                    }

                    Height::Touching(_) if !is_pressed => {
                        mouse.press()?;
                        is_pressed = true;
                    }

                    _ => (),
                }

                if let Some(prev_point) = latest_point {
                    let ndx = new_p.x - prev_point.x;
                    let ndy = new_p.y - prev_point.y;

                    dx += ndx * RM_MAX_X * SCALE;
                    dy -= ndy * RM_MAX_Y * SCALE;

                    if 1.0 <= dx.abs() {
                        mouse.move_rel_x(dx as i32).context("moving mouse")?;
                        dx = 0.;
                    }

                    if 1.0 <= dy.abs() {
                        mouse.move_rel_y(dy as i32).context("moving mouse")?;
                        dy = 0.;
                    }
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

pub struct MouseController {
    device: VirtualDevice,
}

impl Default for MouseController {
    fn default() -> Self {
        let mut device = VirtualDeviceBuilder::new()
            .expect("Creating VirtualDeviceBuilder")
            .name("remarkable-tablet")
            .with_keys(&AttributeSet::from_iter([Key::BTN_LEFT]))
            .expect("setting supported keys")
            .with_relative_axes(&AttributeSet::from_iter([
                RelativeAxisType::REL_X,
                RelativeAxisType::REL_Y,
                RelativeAxisType::REL_WHEEL,
                RelativeAxisType::REL_HWHEEL,
            ]))
            .expect("Setting supported axis")
            .build()
            .unwrap();

        for path in device.enumerate_dev_nodes_blocking().expect("dev nodes") {
            let path = path.expect("getting path");
            println!("Available as {}", path.display());
        }

        Self { device }
    }
}

const CODE_REL_X: u16 = 0x00;
const CODE_REL_Y: u16 = 0x01;

impl MouseController {
    pub fn move_rel_x(&mut self, x: i32) -> Result<()> {
        self.device
            .emit(&[evdev::InputEvent::new_now(
                evdev::EventType::RELATIVE,
                CODE_REL_X,
                x,
            )])
            .context("moving mouse")?;

        Ok(())
    }

    pub fn move_rel_y(&mut self, y: i32) -> Result<()> {
        self.device
            .emit(&[evdev::InputEvent::new_now(
                evdev::EventType::RELATIVE,
                CODE_REL_Y,
                y,
            )])
            .context("moving mouse")?;

        Ok(())
    }

    pub fn move_rel(&mut self, x: i32, y: i32) -> Result<()> {
        self.device
            .emit(&[
                evdev::InputEvent::new_now(evdev::EventType::RELATIVE, CODE_REL_X, x),
                evdev::InputEvent::new_now(evdev::EventType::RELATIVE, CODE_REL_Y, y),
            ])
            .context("moving mouse")?;

        Ok(())
    }

    pub fn press(&mut self) -> Result<()> {
        println!("pressing mouse");
        self.device
            .emit(&[evdev::InputEvent::new_now(
                evdev::EventType::KEY,
                evdev::Key::BTN_LEFT.0,
                1,
            )])
            .context("pressing mouse")?;

        Ok(())
    }

    pub fn release(&mut self) -> Result<()> {
        println!("releasing mouse");
        self.device
            .emit(&[
                evdev::InputEvent::new_now(evdev::EventType::KEY, evdev::Key::BTN_LEFT.0, 0),
                evdev::InputEvent::new_now(
                    evdev::EventType::SYNCHRONIZATION,
                    evdev::Synchronization::SYN_REPORT.0,
                    0,
                ),
            ])
            .context("releasing mouse")?;

        Ok(())
    }
}
