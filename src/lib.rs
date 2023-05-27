use std::{convert::TryFrom, fmt, path::Path};

mod raw_event;
mod tool_events;

use raw_event::{RawEvent, RawEventSource};

pub use tool_events::{ToolEvent, ToolEventSource};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// UnfinishedEvent occurs when a Sync is received
    /// but we've not yet received X and Y coords.

    #[error("Cool not construct an initial ToolEvent of kind `{kind}`: {error}")]
    UnfinishedTool { kind: ToolKind, error: &'static str },

    /// UnknownEvent occurs when the input_event is unknown
    /// and not (yet?) supported by this library.
    #[error("Read an unknown event: {0}")]
    UnknownEventRead(#[from] UnknownEvent),

    /// Unexpected event
    #[error("Unexpected event: {0:?}")]
    UnexpectedEvent(String),

    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
}

pub struct EventSource {
    raw_events: RawEventSource,
}

impl EventSource {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let raw_events = RawEventSource::open(path).await?;
        Ok(Self { raw_events })
    }

    pub async fn next(&mut self) -> Result<Event, Error> {
        let ev = self.raw_events.next().await?;

        Ok(Event::try_from(ev)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Tool {
    pub kind: ToolKind,
    pub point: Point,
    pub tilt_x: Option<i32>,
    pub tilt_y: Option<i32>,
    pub height: Height,
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Tool at {}. tilt x{:?} y{:?}. {}",
            self.point, self.tilt_x, self.tilt_y, self.height
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Height {
    Missing,
    Distance(u32),
    Touching(u32),
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("distance missing"),
            Self::Distance(n) => write!(f, "distance{}", n),
            Self::Touching(n) => write!(f, "touching{}", n),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum ToolKind {
    Pen,
    Rubber,
    Touch,
    Stylus,
    Stylus2,
}

impl fmt::Display for ToolKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Pen => f.write_str("Pen"),
            Self::Rubber => f.write_str("Rubber"),
            Self::Touch => f.write_str("Touch"),
            Self::Stylus => f.write_str("Stylus"),
            Self::Stylus2 => f.write_str("Stylus2"),
        }
    }
}

impl ToolKind {
    fn from_code(code: u16) -> Option<ToolKind> {
        match code {
            320 => Some(Self::Pen),
            321 => Some(Self::Rubber),
            330 => Some(Self::Touch),
            331 => Some(Self::Stylus),
            332 => Some(Self::Stylus2),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Movement {
    X(u32),
    Y(u32),
    TiltX(i32),
    TiltY(i32),
    Pressure(u32),
    Distance(u32),
}

// More Movement, needed to parse events from /dev/input/event2
// #define ABS_MT_TOUCH_MAJOR  0x30    /* Major axis of touching ellipse */
// #define ABS_MT_TOUCH_MINOR  0x31    /* Minor axis (omit if circular) */
// #define ABS_MT_WIDTH_MAJOR  0x32    /* Major axis of approaching ellipse */
// #define ABS_MT_WIDTH_MINOR  0x33    /* Minor axis (omit if circular) */
// #define ABS_MT_ORIENTATION  0x34    /* Ellipse orientation */
// #define ABS_MT_POSITION_X   0x35    /* Center X touch position */
// #define ABS_MT_POSITION_Y   0x36    /* Center Y touch position */
// #define ABS_MT_TOOL_TYPE    0x37    /* Type of touching device */
// #define ABS_MT_BLOB_ID      0x38    /* Group a set of packets as a blob */
// #define ABS_MT_TRACKING_ID  0x39    /* Unique ID of initiated contact */
// #define ABS_MT_PRESSURE     0x3a    /* Pressure on contact area */
// #define ABS_MT_DISTANCE     0x3b    /* Contact hover distance */
// #define ABS_MT_TOOL_X       0x3c    /* Center X tool position */
// #define ABS_MT_TOOL_Y       0x3d    /* Center Y tool position */
//
//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Events read from event1 (Only registers the pen)
pub enum Event {
    Sync,
    ToolAdded(ToolKind),
    ToolRemoved(ToolKind),
    Movement(Movement),
}

impl TryFrom<RawEvent> for Event {
    type Error = UnknownEvent;

    fn try_from(ev: RawEvent) -> std::result::Result<Self, Self::Error> {
        match (ev.typ, ev.code) {
            (0, _) => Ok(Event::Sync),
            (1, code) => {
                // type 1 is a tool event.
                // the code tells what tool
                // value tells if it appeared or went away
                let tool = ToolKind::from_code(code).ok_or(UnknownEvent::ToolCode(code))?;
                match ev.value {
                    0 => Ok(Event::ToolRemoved(tool)),
                    1 => Ok(Event::ToolAdded(tool)),
                    v => Err(UnknownEvent::ToolValue(v)),
                }
            }

            (3, 0) => Ok(Event::Movement(Movement::Y(ev.value))),
            (3, 1) => Ok(Event::Movement(Movement::X(ev.value))),
            (3, 24) => Ok(Event::Movement(Movement::Pressure(ev.value))),
            (3, 25) => Ok(Event::Movement(Movement::Distance(ev.value))),
            (3, 26) => Ok(Event::Movement(Movement::TiltX(ev.value as i32))),
            (3, 27) => Ok(Event::Movement(Movement::TiltY(ev.value as i32))),

            (3, _) => Err(UnknownEvent::MovementCode(ev.code)),

            _ => Err(UnknownEvent::Type(ev)),
        }
    }
}

//

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum UnknownEvent {
    #[error("Unknown ToolCode `{0}`")]
    ToolCode(u16),
    #[error("Unknown ToolValue `{0}`. Should be [0, 1]")]
    ToolValue(u32),
    #[error("Unknown MovementCode `{0}`")]
    MovementCode(u16),

    #[error("Unknown Type `{0:?}`")]
    Type(RawEvent),
}
