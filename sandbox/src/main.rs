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


trait MidiNoteSink {
    fn receive(&mut self, note: &Note);
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
    fn receive(&mut self, n: &Note) {
        self.midi_out.message(&[0x90, n.note, n.velocity]);
        self.midi_out.message(&[0x80, n.note, 0]);
    }
}


// HoldingThru

struct HoldingThru {
    midi_out: RtMidiOut,
    last_note: u8
}

impl HoldingThru {
    pub fn using_device(d: u32) -> HoldingThru {
        let t = HoldingThru {
            midi_out: RtMidiOut::new(Default::default()).unwrap(),
            last_note: 0
        };
        t.midi_out.open_port(d, "HoldingThru out");
        t
    }
}

impl MidiNoteSink for HoldingThru {
    fn receive(&mut self, n: &Note) {
        let same = self.last_note == n.note;
        if self.last_note != 0 || same {
            self.midi_out.message(&[0x80, self.last_note, 0]);
            self.last_note = 0;
        }
        if !same {
            self.midi_out.message(&[0x90, n.note, n.velocity]);
            self.last_note = n.note;
        }
    }
}

impl Drop for HoldingThru {
    fn drop(&mut self) {
        if self.last_note != 0 {
            self.midi_out.message(&[0x80, self.last_note, 0]);
        }
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
    let mut thru = Mutex::new(HoldingThru::using_device(2));

    input.set_callback(|timestamp, message| {
        let n = Note::from_midi_message(&message);
        if n.velocity != 0 {
            println!("{:02x} {} {}", message[0], n.note, n.velocity);
            thru.lock().unwrap().receive(&n);
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
