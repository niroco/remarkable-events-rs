use std::{convert::TryFrom, error::Error as StdError, fmt, path::Path};

use event_source::EventSourceError;

mod event_source;
mod raw_event;

use {event_source::EventSource, raw_event::RawEvent};

#[derive(Debug)]
pub enum Error {
    /// UnfinishedEvent occurs when a Sync is received
    /// but we've not yet received X and Y coords.
    UnfinishedEvent(ToolKind, &'static str),

    /// UnknownEvent occurs when the input_event is unknown
    /// and not (yet?) supported by this library.
    UnknownEventRead(UnknownEvent),

    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UnfinishedEvent(kind, missing) => {
                write!(f, "Unfinished {} event. Missing {}", kind, missing)
            }
            Self::UnknownEventRead(err) => write!(f, "Read Unknown event: {}", err),

            Self::Io(err) => write!(f, "Io error: {}", err),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(ref inner) => inner.source(),
            _ => None,
        }
    }
}

impl From<EventSourceError<UnknownEvent>> for Error {
    fn from(err: EventSourceError<UnknownEvent>) -> Self {
        match err {
            EventSourceError::Io(io_err) => Self::Io(io_err),
            EventSourceError::Parse(ev_err) => Self::UnknownEventRead(ev_err),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ToolEvent {
    Update(Tool),
    Removed(ToolKind),
}
/// This should be generalised by making struct Pen
/// be a struct Tool that has a kind field.
/// For now, to get stuff going, just run with it as a Pen.
pub struct ToolEventSource {
    event_source: EventSource<Event>,
}

impl ToolEventSource {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let event_source = event_source::EventSource::open(path)
            .await
            .map_err(Error::Io)?;

        Ok(Self { event_source })
    }

    pub async fn next(&mut self) -> Result<ToolEvent, Error> {
        // Currently we need to listen to Tool events.
        // Tool::Pen(true) means the Pen is close to the Pad, start to build.
        // Tool::Pen(false) means the Pen was lifted and we need to reset.
        //
        // These events dictate what we should do.
        // @TODO: Handle these events properly.

        let mut start_kind: Option<ToolKind> = None;

        let kind = loop {
            match self.event_source.next().await? {
                Event::ToolAdded(kind) => {
                    start_kind = Some(kind);
                }

                Event::Sync if start_kind.is_some() => {
                    break start_kind.unwrap();
                }

                Event::Sync => {
                    eprintln!("Got a sync without received ToolAdded");
                }

                ev => eprintln!("Skipping {ev:?}. Waiting for ToolAdded"),
            }
        };

        let mut builder = UnfinishedTool::new(kind);

        loop {
            match self.event_source.next().await? {
                Event::Movement(mv) => builder.apply_movement(mv),

                Event::Sync => {
                    let tool = builder
                        .finish()
                        .map_err(|err| Error::UnfinishedEvent(builder.kind, err))?;

                    return Ok(ToolEvent::Update(tool));
                }

                Event::ToolRemoved(kind) => {
                    // Should really wait for a sync....
                    eprintln!("Ignoring non Pen ToolRemoved({:?})", kind);
                }

                Event::ToolAdded(kind) => {
                    eprintln!("Ignoring ToolAdded({:?})", kind);
                }
            }
        }
    }
}

struct UnfinishedTool {
    kind: ToolKind,
    x: Option<u32>,
    y: Option<u32>,
    tilt_x: Option<i32>,
    tilt_y: Option<i32>,
    pressure: Option<u32>,
    distance: Option<u32>,
}

impl UnfinishedTool {
    fn new(kind: ToolKind) -> Self {
        Self {
            kind,
            x: None,
            y: None,
            tilt_x: None,
            tilt_y: None,
            pressure: None,
            distance: None,
        }
    }

    fn apply_movement(&mut self, mv: Movement) {
        match mv {
            Movement::X(n) => self.x = Some(n),
            Movement::Y(n) => self.y = Some(n),
            Movement::TiltX(n) => self.tilt_x = Some(n),
            Movement::TiltY(n) => self.tilt_y = Some(n),
            Movement::Pressure(n) => self.pressure = Some(n),
            Movement::Distance(n) => self.distance = Some(n),
        };
    }

    // @TODO: Change Return type to Result with an error saying what field is missing.
    fn finish(&mut self) -> Result<Tool, &'static str> {
        let x = self.x.take().ok_or("X")?;
        let y = self.y.take().ok_or("Y")?;

        let height = match (self.pressure.take(), self.distance.take()) {
            (Some(pressure), Some(distance)) => {
                if distance < 10 && 700 < pressure {
                    Height::Touching(pressure)
                } else {
                    Height::Distance(distance)
                }
            }
            (Some(pressure), None) => Height::Touching(pressure),
            (None, Some(distance)) => Height::Distance(distance),
            (None, None) => Height::Missing,
        };

        Ok(Tool {
            kind: self.kind,
            point: Point(x, y),
            tilt_x: self.tilt_x.take(),
            tilt_y: self.tilt_y.take(),
            height,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Tool {
    kind: ToolKind,
    point: Point,
    tilt_x: Option<i32>,
    tilt_y: Option<i32>,
    height: Height,
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
pub struct Point(u32, u32);

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{}", self.0, self.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
enum Event {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownEvent {
    ToolCode(u16),
    ToolValue(u32),
    MovementCode(u16),

    Type(RawEvent),
}

impl std::error::Error for UnknownEvent {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl fmt::Display for UnknownEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ToolCode(code) => write!(f, "Unknown tool code`{:#04x}`", code),
            Self::ToolValue(value) => write!(
                f,
                "Unexpected value for Tool event. Should be 0 or 1. Was:`{:#04x}`",
                value
            ),
            Self::MovementCode(code) => write!(f, "Unknown movement code `{:#04x}`", code),
            Self::Type(ev) => write!(
                f,
                "Unknown type: `{:#02x}`, code: `{:#04x}`, value: `{:#08x}`, ",
                ev.typ, ev.code, ev.value
            ),
        }
    }
}
