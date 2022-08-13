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
    note: u8
}


impl NoteStats {
    fn new() -> NoteStats {
        NoteStats {
            note: 0
        }
    }

    fn last_note(&self) -> Option<u8> {
        match self.note {
            0 => None,
            n => Some(n)
        }
    }

    fn add(&mut self, n: &Note) {
        if self.note != n.note {
            self.note = n.note;
        } else {
            self.note = 0;
        }
    }
}



trait MidiNoteSink {
    fn receive(&self, note: &Note, stats: &NoteStats);
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
    fn receive(&self, n: &Note, _stats: &NoteStats) {
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
    fn receive(&self, n: &Note, stats: &NoteStats) {
        
        let transposed = Note {
            note: self.tonic + n.note,
            velocity: n.velocity
        };
        
        self.next.receive(&transposed, &stats);
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
    fn receive(&self, n: &Note, stats: &NoteStats) {
        if !stats.last_note().is_none() {
            self.midi_out.message(&[0x80, stats.last_note().unwrap(), 0]);
        }

        if stats.last_note().is_none() || stats.last_note().unwrap() != n.note {
            self.midi_out.message(&[0x90, n.note, n.velocity]);
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
    let mut stats = Mutex::new(NoteStats::new());
    let thru = HoldingThru::using_device(2);

    input.set_callback(|_timestamp, message| {
        let n = Note::from_midi_message(&message);
        if n.velocity != 0 {
            println!("{:02x} {} {}", message[0], n.note, n.velocity);

            let mut s = stats.lock().unwrap();
            thru.receive(&n, &s);
            s.add(&n);
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
