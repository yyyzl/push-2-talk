use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

pub fn get_cursor_position() -> Option<(i32, i32)> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let location = event.location();

    Some((location.x.round() as i32, location.y.round() as i32))
}
