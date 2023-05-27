use std::path::Path;

use crate::{Error, Event, EventSource, Height, Movement, Tool, ToolKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ToolEvent {
    Update(Tool),
    Removed,
}
/// This should be generalised by making struct Pen
/// be a struct Tool that has a kind field.
/// For now, to get stuff going, just run with it as a Pen.
pub struct ToolEventSource {
    events: EventSource,
    syncs_to_ignore: usize,
    builder: ToolBuilder,
}

impl ToolEventSource {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let events = EventSource::open(path).await?;

        Ok(Self {
            events,
            syncs_to_ignore: 0,
            builder: ToolBuilder::default(),
        })
    }

    pub async fn next(&mut self) -> Result<ToolEvent, Error> {
        // Currently we need to listen to Tool events.
        // Tool::Pen(true) means the Pen is close to the Pad, start to build.
        // Tool::Pen(false) means the Pen was lifted and we need to reset.
        //
        // These events dictate what we should do.
        // @TODO: Handle these events properly.

        loop {
            match self.events.next().await? {
                // We ignored Any Adds or Removes for Tuch.
                // since we determine if the Pen is touching or not
                // depending on the Height (Distance or Pressure).
                Event::ToolAdded(ToolKind::Touch) => (),
                Event::ToolRemoved(ToolKind::Touch) => (),

                Event::Movement(mv) => self.builder.apply_movement(mv),

                Event::ToolRemoved(_) => {
                    // Ignore one event since we emit the event now.
                    self.syncs_to_ignore += 1;
                    self.builder.kind = None;
                    return Ok(ToolEvent::Removed);
                }

                Event::Sync if 0 < self.syncs_to_ignore => {
                    self.syncs_to_ignore -= 1;
                }

                Event::Sync => match self.builder.construct() {
                    Ok(tool) => return Ok(ToolEvent::Update(tool)),

                    Err(err) => {
                        eprintln!("Constructing on Sync: {err}");
                    }
                },

                Event::ToolAdded(kind) => {
                    self.builder.reset(kind);
                }
            }
        }
    }
}

#[derive(Default)]
struct ToolBuilder {
    kind: Option<ToolKind>,
    x: Option<u32>,
    y: Option<u32>,
    tilt_x: Option<i32>,
    tilt_y: Option<i32>,
    pressure: Option<u32>,
    distance: Option<u32>,
}

impl ToolBuilder {
    fn reset(&mut self, kind: ToolKind) {
        self.kind = Some(kind);
        self.x = None;
        self.y = None;
        self.tilt_y = None;
        self.tilt_x = None;
        self.pressure = None;
        self.distance = None;
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
    fn construct(&mut self) -> Result<Tool, &'static str> {
        let x = self.x.ok_or("X")?;
        let y = self.y.ok_or("Y")?;
        let kind = self.kind.ok_or("kind")?;

        let height = match (self.pressure, self.distance) {
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
            kind,
            point: crate::Point(x, y),
            tilt_x: self.tilt_x,
            tilt_y: self.tilt_y,
            height,
        })
    }
}
