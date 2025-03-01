#![crate_name = "coremidi"]
#![crate_type = "lib"]
#![doc(html_root_url = "https://chris-zen.github.io/coremidi/")]

/*!
This is a [CoreMIDI](https://developer.apple.com/documentation/coremidi) library for Rust built on top of the low-level bindings [coremidi-sys](https://github.com/jonas-k/coremidi-sys).
CoreMIDI is a macOS framework that provides APIs for communicating with MIDI (Musical Instrument Digital Interface) devices, including hardware keyboards and synthesizers.

This library preserves the fundamental concepts behind the CoreMIDI framework, while being Rust idiomatic. This means that if you already know CoreMIDI, you will find very easy to start using it.

Please see the [examples](https://github.com/chris-zen/coremidi/tree/master/examples) for getting an idea of how it looks like, but if you are eager to see an example, this is how you would send some note:

```rust,no_run
use coremidi::{Client, Destination, EventBuffer, Protocol};
use std::time::Duration;
use std::thread;

fn main() {
    let client = coremidi::Client::new("example-client").unwrap();
    let output_port = client.output_port("example-port").unwrap();
    let destination = Destination::from_index(0).unwrap();
    let note_on = EventBuffer::new(Protocol::Midi10).with_packet(0, &[0x2090407f]);
    let note_off = EventBuffer::new(Protocol::Midi10).with_packet(0, &[0x2080407f]);
    output_port.send(&destination, &note_on).unwrap();
    thread::sleep(Duration::from_millis(1000));
    output_port.send(&destination, &note_off).unwrap();
}
```

If you are looking for a portable MIDI library then you can look into:

- [midir](https://github.com/Boddlnagg/midir) (which is using this lib)
- [portmidi-rs](https://github.com/musitdev/portmidi-rs)

For handling low level MIDI data you may look into:

- [midi-rs](https://github.com/samdoshi/midi-rs)
- [rimd](https://github.com/RustAudio/rimd)

*/

mod client;
mod device;
mod endpoints;
mod entity;
mod events;
mod notifications;
mod object;
mod packets;
mod ports;
mod properties;
mod protocol;

use core_foundation_sys::base::OSStatus;

use coremidi_sys::{MIDIFlushOutput, MIDIRestart};

pub use crate::client::{Client, NotifyCallback};
pub use crate::device::Device;
pub use crate::endpoints::destinations::{Destination, Destinations, VirtualDestination};
pub use crate::endpoints::endpoint::Endpoint;
pub use crate::endpoints::sources::{Source, Sources, VirtualSource};
pub use crate::entity::Entity;
pub use crate::events::{EventBuffer, EventList, EventListIter, EventPacket, Timestamp};
pub use crate::notifications::{AddedRemovedInfo, IoErrorInfo, Notification, PropertyChangedInfo};
pub use crate::object::{Object, ObjectType};
pub use crate::packets::{Packet, PacketBuffer, PacketList, PacketListIterator};
pub use crate::ports::{InputPort, InputPortWithContext, OutputPort};
pub use crate::properties::{
    BooleanProperty, IntegerProperty, Properties, PropertyGetter, PropertySetter, StringProperty,
};
pub use crate::protocol::Protocol;

/// Unschedules previously-sent packets for all the endpoints.
/// See [MIDIFlushOutput](https://developer.apple.com/documentation/coremidi/1495312-midiflushoutput).
///
pub fn flush() -> Result<(), OSStatus> {
    let status = unsafe { MIDIFlushOutput(0) };
    unit_result_from_status(status)
}

/// Stops and restarts MIDI I/O.
/// See [MIDIRestart](https://developer.apple.com/documentation/coremidi/1495146-midirestart).
///
pub fn restart() -> Result<(), OSStatus> {
    let status = unsafe { MIDIRestart() };
    unit_result_from_status(status)
}

/// Convert an OSStatus into a Result<T, OSStatus> given a mapping closure
fn result_from_status<T, F: FnOnce() -> T>(status: OSStatus, f: F) -> Result<T, OSStatus> {
    match status {
        0 => Ok(f()),
        _ => Err(status),
    }
}

/// Convert an OSSStatus into a Result<(), OSStatus>
fn unit_result_from_status(status: OSStatus) -> Result<(), OSStatus> {
    result_from_status(status, || ())
}
