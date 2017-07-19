use coremidi_sys::{
    MIDITimeStamp, UInt16, MIDIPacketList, MIDIPacket, MIDIPacketListInit, MIDIPacketNext
};

use std::fmt;
use std::ops::Deref;

use PacketList;
use AlignmentMarker;

pub type Timestamp = u64;

const MAX_PACKET_DATA_LENGTH: usize = 0xffffusize;

/// A collection of simultaneous MIDI events.
/// See [MIDIPacket](https://developer.apple.com/reference/coremidi/midipacket).
///
#[repr(C)]
pub struct Packet {
    // NOTE: At runtime this type must only be used behind immutable references
    //       that point to valid instances of MIDIPacket (mutable references would allow mem::swap).
    //       This type must NOT implement `Copy`!
    //       On ARM, this must be 4-byte aligned.
    inner: PacketInner,
    _do_not_construct: AlignmentMarker
}

#[repr(packed)]
struct PacketInner {
    timestamp: MIDITimeStamp,
    length: u16,
    data: [u8; 0], // zero-length, because we cannot make this type bigger without knowing how much data there actually is
}

impl Packet {
    /// Get the packet timestamp.
    ///
    pub fn timestamp(&self) -> Timestamp {
        self.inner.timestamp as Timestamp
    }

    /// Get the packet data. This method just gives raw MIDI bytes. You would need another
    /// library to decode them and work with higher level events.
    ///
    ///
    /// The following example:
    ///
    /// ```
    /// let packet_list = &coremidi::PacketBuffer::from_data(0, vec![0x90, 0x40, 0x7f]);
    /// for packet in packet_list.iter() {
    ///   for byte in packet.data() {
    ///     print!(" {:x}", byte);
    ///   }
    /// }
    /// ```
    ///
    /// will print:
    ///
    /// ```text
    ///  90 40 7f
    /// ```
    pub fn data(&self) -> &[u8] {
        let data_ptr = self.inner.data.as_ptr();
        let data_len = self.inner.length as usize;
        unsafe { ::std::slice::from_raw_parts(data_ptr, data_len) }
    }
}

impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = write!(f, "Packet(ptr={:x}, ts={:016x}, data=[",
                            self as *const _ as usize, self.timestamp() as u64);
        let result = self.data().iter().enumerate().fold(result, |prev_result, (i, b)| {
            match prev_result {
                Err(err) => Err(err),
                Ok(()) => {
                    let sep = if i > 0 { ", " } else { "" };
                    write!(f, "{}{:02x}", sep, b)
                }
            }
        });
        result.and_then(|_| write!(f, "])"))
    }
}

impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = write!(f, "{:016x}:", self.timestamp());
        self.data().iter().fold(result, |prev_result, b| {
            match prev_result {
                Err(err) => Err(err),
                Ok(()) => write!(f, " {:02x}", b)
            }
        })
    }
}

impl PacketList {

    /// Get the number of packets in the list.
    ///
    pub fn length(&self) -> usize {
        self.inner.num_packets as usize
    }

    /// Get an iterator for the packets in the list.
    ///
    pub fn iter<'a>(&'a self) -> PacketListIterator<'a> {
        PacketListIterator {
            count: self.length(),
            packet_ptr: self.inner.data.as_ptr(),
            _phantom: ::std::marker::PhantomData::default(),
        }
    }
}

impl fmt::Debug for PacketList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = write!(f, "PacketList(ptr={:x}, packets=[", unsafe { self.as_ptr() as usize });
        self.iter().enumerate().fold(result, |prev_result, (i, packet)| {
            match prev_result {
                Err(err) => Err(err),
                Ok(()) => {
                    let sep = if i != 0 { ", " } else { "" };
                    write!(f, "{}{:?}", sep, packet)
                }
            }
        }).and_then(|_| write!(f, "])"))
    }
}

impl fmt::Display for PacketList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = write!(f, "PacketList(len={})", self.inner.num_packets);
        self.iter().fold(result, |prev_result, packet| {
            match prev_result {
                Err(err) => Err(err),
                Ok(()) => write!(f, "\n  {}", packet)
            }
        })
    }
}

pub struct PacketListIterator<'a> {
    count: usize,
    packet_ptr: *const MIDIPacket,
    _phantom: ::std::marker::PhantomData<&'a Packet>,
}

impl<'a> Iterator for PacketListIterator<'a> {
    type Item = &'a Packet;

    fn next(&mut self) -> Option<&'a Packet> {
        if self.count > 0 {
            let packet = unsafe { &*(self.packet_ptr as *const Packet) };
            self.count -= 1;
            self.packet_ptr = unsafe { MIDIPacketNext(self.packet_ptr) };
            Some(packet)
        }
        else {
            None
        }
    }
}

const PACKET_LIST_HEADER_SIZE: usize = 4;  // MIDIPacketList::numPackets: UInt32
const PACKET_HEADER_SIZE: usize = 8 +      // MIDIPacket::timeStamp: MIDITimeStamp/UInt64
                                  2;       // MIDIPacket::length: UInt16

/// A mutable `PacketList` builder.
///
/// A `PacketList` is an inmmutable reference to a [MIDIPacketList](https://developer.apple.com/reference/coremidi/midipacketlist) structure,
/// while a `PacketBuffer` is a mutable structure that allows to build a `PacketList` by adding packets.
/// It dereferences to a `PacketList`, so it can be used whenever a `PacketList` is needed.
///
pub struct PacketBuffer {
    data: Vec<u8>
}

impl PacketBuffer {
    /// Create an empty `PacketBuffer`.
    ///
    pub fn new() -> PacketBuffer {
        let capacity = PACKET_LIST_HEADER_SIZE + PACKET_HEADER_SIZE + 3;
        let mut data = Vec::<u8>::with_capacity(capacity);
        unsafe { data.set_len(PACKET_LIST_HEADER_SIZE) };
        let pkt_list_ptr = data.as_mut_ptr() as *mut MIDIPacketList;
        let _ = unsafe { MIDIPacketListInit(pkt_list_ptr) };
        PacketBuffer {
            data: data
        }
    }

    /// Create a `PacketBuffer` with a single packet containing the provided timestamp and data.
    ///
    /// According to the official documentation for CoreMIDI, the timestamp represents
    /// the time at which the events are to be played, where zero means "now".
    /// The timestamp applies to the first MIDI byte in the packet.
    ///
    /// Example on how to create a `PacketBuffer` with a single packet for a MIDI note on for C-5:
    ///
    /// ```
    /// let note_on = coremidi::PacketBuffer::from_data(0, vec![0x90, 0x3c, 0x7f]);
    /// ```
    #[inline]
    pub fn from_data(timestamp: Timestamp, data: Vec<u8>) -> PacketBuffer {
        Self::new().with_data(timestamp, data)
    }

    /// Add a new packet containing the provided timestamp and data.
    ///
    /// According to the official documentation for CoreMIDI, the timestamp represents
    /// the time at which the events are to be played, where zero means "now".
    /// The timestamp applies to the first MIDI byte in the packet.
    ///
    /// Example:
    ///
    /// ```
    /// let chord = coremidi::PacketBuffer::new()
    ///   .with_data(0, vec![0x90, 0x3c, 0x7f])
    ///   .with_data(0, vec![0x90, 0x40, 0x7f]);
    /// println!("{}", &chord as &coremidi::PacketList);
    /// ```
    ///
    /// The previous example should print:
    ///
    /// ```text
    /// PacketList(len=2)
    ///   0000000000000000: 90 3c 7f
    ///   0000000000000000: 90 40 7f
    /// ```
    pub fn with_data(mut self, timestamp: Timestamp, data: Vec<u8>) -> Self {
        let data_len = data.len();
        assert!(data_len < MAX_PACKET_DATA_LENGTH,
                "The maximum allowed size for a packet is {}, but found {}.",
                MAX_PACKET_DATA_LENGTH, data_len);

        let additional_size = PACKET_HEADER_SIZE + data_len;
        self.data.reserve(additional_size);

        let mut pkt = unsafe {
            let total_len = self.data.len();
            self.data.set_len(total_len + additional_size);
            &mut *(&self.data[total_len] as *const _ as *mut MIDIPacket)
        };

        pkt.timeStamp = timestamp as MIDITimeStamp;
        pkt.length = data_len as UInt16;
        pkt.data[0..data_len].clone_from_slice(&data);

        let mut pkt_list = unsafe { &mut *(self.data.as_mut_ptr() as *mut MIDIPacketList) };
        pkt_list.numPackets += 1;

        self
    }
}

impl Deref for PacketBuffer {
    type Target = PacketList;

    fn deref(&self) -> &PacketList {
        unsafe { &*(self.data.as_ptr() as *const PacketList) }
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use coremidi_sys::{MIDITimeStamp, MIDIPacketList};
    use PacketList;
    use PacketBuffer;
    use Packet;
    use super::{PACKET_HEADER_SIZE, PACKET_LIST_HEADER_SIZE};

    #[test]
    pub fn packet_struct_layout() {
        let expected_align = if cfg!(any(target_arch = "arm", target_arch = "aarch64")) { 4 } else { 1 };
        assert_eq!(expected_align, mem::align_of::<Packet>());
        assert_eq!(expected_align, mem::align_of::<PacketList>());

        let dummy_packet: Packet = unsafe { mem::zeroed() };
        let ptr = &dummy_packet as *const _ as *const u8;
        assert_eq!(PACKET_HEADER_SIZE, dummy_packet.inner.data.as_ptr() as usize - ptr as usize);

        let dummy_packet_list: PacketList = unsafe { mem::zeroed() };
        let ptr = &dummy_packet_list as *const _ as *const u8;
        assert_eq!(PACKET_LIST_HEADER_SIZE, dummy_packet_list.inner.data.as_ptr() as usize - ptr as usize);
    }

    #[test]
    pub fn packet_buffer_new() {
        let packet_buf = PacketBuffer::new();
        assert_eq!(packet_buf.data.len(), 4);
        assert_eq!(packet_buf.data, vec![0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    pub fn packet_buffer_with_data() {
        let packet_buf = PacketBuffer::new()
            .with_data(0x0102030405060708 as MIDITimeStamp, vec![0x90u8, 0x40, 0x7f]);
        assert_eq!(packet_buf.data.len(), 17);
        // FIXME This is platform endianess dependent
        assert_eq!(packet_buf.data, vec![
            0x01, 0x00, 0x00, 0x00,
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
            0x03, 0x00,
            0x90, 0x40, 0x7f]);
    }

    #[test]
    fn packet_buffer_deref() {
        let packet_buf = PacketBuffer::new();
        let packet_list: &PacketList = &packet_buf;
        assert_eq!(unsafe { packet_list.as_ptr() as *const MIDIPacketList }, &packet_buf.data[0] as *const _ as *const MIDIPacketList);
    }

    #[test]
    fn packet_list_length() {
        let packet_buf = PacketBuffer::new()
            .with_data(0, vec![0x90u8, 0x40, 0x7f])
            .with_data(0, vec![0x91u8, 0x40, 0x7f])
            .with_data(0, vec![0x80u8, 0x40, 0x7f])
            .with_data(0, vec![0x81u8, 0x40, 0x7f]);
        assert_eq!(packet_buf.length(), 4);
    }

    #[test]
    fn compare_with_native1() {
        unsafe { build_packet_list(vec![
            (0, vec![0x90, 0x40, 0x7f]),
            (0, vec![0x90, 0x41, 0x7f]),
            (0, vec![0x90, 0x42, 0x7f])
        ]) }
    }

    #[test]
    fn compare_with_native2() {
        unsafe { build_packet_list(vec![
            (0, vec![0x90, 0x40, 0x7f]),
            (1, vec![0x90, 0x40, 0x7f]),
            (2, vec![0x90, 0x40, 0x7f])
        ]) }
    }

    #[test]
    fn compare_with_native3() {
        let mut sysex = vec![0xF0];
        for _ in 0..300 {
            sysex.push(0x00);
        }
        sysex.push(0xF7);
        unsafe { build_packet_list(vec![
            (0, vec![0x90, 0x40, 0x7f]),
            (0, vec![0x90, 0x41, 0x7f]),
            (0, sysex)
        ]) }
    }
    
    unsafe fn build_packet_list(packets: Vec<(MIDITimeStamp, Vec<u8>)>) {
        use coremidi_sys::{MIDIPacketList, MIDIPacketListInit, MIDIPacketListAdd};

        // allocate a buffer on the stack for building the list using native methods
        const BUFFER_SIZE: usize = 65536; // maximum allowed size
        let buffer: &mut [u8] = &mut [0; BUFFER_SIZE];
        let pkt_list_ptr = buffer.as_mut_ptr() as *mut MIDIPacketList;

        // build the list
        let mut pkt_ptr = MIDIPacketListInit(pkt_list_ptr);
        for pkt in &packets {
            pkt_ptr = MIDIPacketListAdd(pkt_list_ptr, BUFFER_SIZE as u64, pkt_ptr, pkt.0, pkt.1.len() as u64, pkt.1.as_ptr());
            assert!(!pkt_ptr.is_null());
        }
        let list_native = &*(pkt_list_ptr as *const _ as *const PacketList);

        // build the PacketBuffer, containing the same packets
        /*let mut packet_buf = PacketBuffer::new();
        for pkt in &packets {
            packet_buf = packet_buf.with_data(pkt.0, pkt.1.clone());
        }
        let list: &PacketList = &packet_buf;

        // check if the contents match
        assert_eq!(list_native.length(), list.length());
        // TODO: check if data matches
        */

        assert_eq!(packets.len(), list_native.length());
    }
}
