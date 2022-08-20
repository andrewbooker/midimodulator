use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};


struct Note {
    note: u8,
    velocity: u8
}

impl Note {
    pub fn from_midi_message(message: &[u8]) -> Note {
        Note {
            note: message[1],
            velocity: message[2]
        }
    }
}

struct NoteStats {
    received: u8,
    sent: u8
}


impl NoteStats {
    fn new() -> NoteStats {
        NoteStats {
            received: 0,
            sent: 0
        }
    }

    fn last_received(&self) -> Option<u8> {
        match self.received {
            0 => None,
            n => Some(n)
        }
    }
    
    fn last_sent(&self) -> Option<u8> {
        match self.sent {
            0 => None,
            n => Some(n)
        }
    }

    fn put_received(&mut self, n: &Note) {
        if self.received != n.note {
            self.received = n.note;
        } else {
            self.received = 0;
        }
    }

    fn put_sent(&mut self, n: &Note) {
        if self.sent != n.note {
            self.sent = n.note;
        } else {
            self.sent = 0;
        }
    }

    fn clear(&mut self) {
        self.sent = 0;
        self.received = 0;
    }
}



trait MidiNoteSink {
    fn receive(&self, note: &Note, stats: &mut NoteStats);
}


struct InputRegister<'a, S: MidiNoteSink> {
    next: &'a S
}

impl <'a, S: MidiNoteSink>InputRegister<'a, S> {
    fn then(next: &'a S) -> InputRegister<'a, S> {
        InputRegister::<'a, S> {
            next: next
        }
    }
}

impl <'a, S: MidiNoteSink>MidiNoteSink for InputRegister<'a, S> {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        println!("got note {} vel {}", n.note, n.velocity);
        stats.put_received(&n);
        self.next.receive(&n, stats);
    }
}


// SimpleThru

struct SimpleThru {
    midi_out: RtMidiOut
}

impl SimpleThru {
    pub fn using_device(d: u32) -> SimpleThru {
        let t = SimpleThru {
            midi_out: RtMidiOut::new(Default::default()).unwrap()
        };
        t.midi_out.open_port(d, "SimpleThru out");
        t
    }
}

impl MidiNoteSink for SimpleThru {
    fn receive(&self, n: &Note, _stats: &mut NoteStats) {
        self.midi_out.message(&[0x90, n.note, n.velocity]);
        self.midi_out.message(&[0x80, n.note, 0]);
    }
}


// NoteMapThru

struct NoteMapThru<'a, S: MidiNoteSink> {
    next: &'a S,
    tonic: u8
}


impl <'a, S: MidiNoteSink>NoteMapThru<'a, S> {
    pub fn to(tonic: u8, next: &'a S) -> NoteMapThru<'a, S> {
        NoteMapThru::<'a, S> {
            tonic,
            next
        }
    }
}


impl <'a, S: MidiNoteSink>MidiNoteSink for NoteMapThru<'a, S> {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let transposed = Note {
            note: self.tonic + n.note,
            velocity: n.velocity
        };
        
        self.next.receive(&transposed, stats);
    }
}   


// HoldingThru

struct HoldingThru {
    midi_out: RtMidiOut
}

impl HoldingThru {
    pub fn using_device(d: u32) -> HoldingThru {
        let t = HoldingThru {
            midi_out: RtMidiOut::new(Default::default()).unwrap()
        };
        t.midi_out.open_port(d, "HoldingThru out");
        t
    }
}

impl MidiNoteSink for HoldingThru {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        if !stats.last_sent().is_none() {
            let ls = stats.last_sent().unwrap();
            self.midi_out.message(&[0x80, ls, 0]);
        }

        if stats.last_sent().is_none() || stats.last_sent().unwrap() != n.note {
            self.midi_out.message(&[0x90, n.note, n.velocity]);
            stats.put_sent(&n);
        } else {
            stats.clear();
        }
    }
}

impl Drop for HoldingThru {
    fn drop(&mut self) {
        self.midi_out.message(&[0xB0, 0x7B, 0]);
        println!("HoldingThru closed");
    }
}


fn main() -> Result<(), RtMidiError> {

    let input = RtMidiIn::new(Default::default())?;
    let input_ports = input.port_count()?;
    println!("{} MIDI input sources", input_ports);
    for port in 0..input_ports {
        println!("Input {}: {}", port + 1, input.port_name(port)?);
    }

    input.open_port(2, "RtMidi Input")?;

    let stats = Mutex::new(NoteStats::new());
    let hold = HoldingThru::using_device(2);
    let mapper = NoteMapThru::to(46, &hold);
    let sink = InputRegister::then(&mapper);

    input.set_callback(|_timestamp, message| {
        let n = Note::from_midi_message(&message);
        if n.velocity != 0 {
            let mut s = stats.lock().unwrap();
            sink.receive(&n, &mut s);
        }
    })?;

    input.ignore_types(true, true, true)?;

    println!("Starting...");

    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();
    thread::spawn(move || {
        let g = getch::Getch::new();
        loop {
            let c: u8 = g.getch().unwrap();
            match c as char {
                'q' => {
                    cmd_stop_tx.send(()).unwrap();
                    break;
                },
                _ => {}
            }
        }
    });

    loop {
        match cmd_stop_rx.try_recv() {
            Ok(_) => {
                println!("stopping...");
                break;
            },
            _ => {}
        }
    }
    
    Ok(())
}
