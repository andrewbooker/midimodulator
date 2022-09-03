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
        t.midi_out.open_port(d, "SimpleThru out").unwrap();
        t
    }
}

impl MidiNoteSink for SimpleThru {
    fn receive(&self, n: &Note, _stats: &mut NoteStats) {
        self.midi_out.message(&[0x90, n.note, n.velocity]).unwrap();
        self.midi_out.message(&[0x80, n.note, 0]).unwrap();
    }
}


// Scale

type Mode = [u8; 6];
const AEOLIAN: Mode = [2, 1, 2, 2, 1, 2];
const LYDIAN: Mode = [2, 2, 2, 1, 2, 2];

const SCALE_LENGTH: usize = 9;

struct Scale {
    notes: [u8; SCALE_LENGTH]
}


impl Scale {
    fn from(tonic: u8, mode: &Mode) -> Scale {
        let mode_len: u8 = mode.len() as u8;

        let mut notes = [0; SCALE_LENGTH];
        let mut octaves: u8 = 0;
        let mut base: u8 = tonic;

        for n in 0..SCALE_LENGTH as u8 {
            if (n % (mode_len + 1)) == 0 {
                base = tonic + (octaves * 12) as u8;
                octaves += 1;
            } else {
                let idx = (n - octaves) % mode_len;
                base += mode[idx as usize];
            }
            notes[n as usize] = base;
        }
        Scale {
            notes
        }
    }

    fn at(&self, idx: u8) -> u8 {
        self.notes[idx as usize]
    }
}


// NoteMapThru

struct NoteMapThru<'a, S: MidiNoteSink> {
    next: &'a S,
    scale: &'a Scale
}

impl <'a, S: MidiNoteSink>NoteMapThru<'a, S> {
    pub fn to(scale: &'a Scale, next: &'a S) -> NoteMapThru<'a, S> {
        NoteMapThru::<'a, S> {
            next,
            scale
        }
    }
}

impl <'a, S: MidiNoteSink>MidiNoteSink for NoteMapThru<'a, S> {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let transposed = Note {
            note: self.scale.at(n.note),
            velocity: n.velocity
        };
        
        self.next.receive(&transposed, stats);
    }
}


// NoteSplitter

struct NoteSplitter<'a, S1: MidiNoteSink, S2: MidiNoteSink> {
    next1: &'a S1,
    next2: &'a S2
}


impl <'a, S1: MidiNoteSink, S2: MidiNoteSink>NoteSplitter<'a, S1, S2> {
    pub fn to(next1: &'a S1, next2: &'a S2) -> NoteSplitter<'a, S1, S2> {
        NoteSplitter::<'a, S1, S2> {
            next1,
            next2
        }
    }
}


impl <'a, S1: MidiNoteSink, S2: MidiNoteSink>MidiNoteSink for NoteSplitter<'a, S1, S2> {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        self.next1.receive(&n, stats);
        self.next2.receive(&n, stats);
    }
}


// RandomOctaveStage

struct RandomOctaveStage<'a, S: MidiNoteSink> {
    octave_range: u8,
    base: i8,
    next: &'a S
}

impl <'a, S: MidiNoteSink>RandomOctaveStage<'a, S> {
    pub fn to(octave_range: u8, base: i8, next: &'a S) -> RandomOctaveStage<'a, S> {
        RandomOctaveStage::<'a, S> {
            octave_range,
            base,
            next
        }
    }
}

impl <'a, S: MidiNoteSink>MidiNoteSink for RandomOctaveStage<'a, S> {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let r = rand::random::<f64>();
        let o = ((r * self.octave_range as f64) as i8) + self.base;
        let transposed = Note {
            note: (n.note as i8 + (12 * o)) as u8,
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
    pub fn using_device(substr: &str) -> HoldingThru {
        let t = HoldingThru {
            midi_out: RtMidiOut::new(Default::default()).unwrap()
        };
        for port in 0..t.midi_out.port_count().unwrap() {
            let name = t.midi_out.port_name(port).unwrap();
            if name.to_lowercase().contains(&substr.to_lowercase()) {
                println!("found output port {} for {} ({})", port, substr, name);
                t.midi_out.open_port(port, "HoldingThru out").unwrap();
            }
        }
        t
    }
}

impl MidiNoteSink for HoldingThru {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        if !stats.last_sent().is_none() {
            let ls = stats.last_sent().unwrap();
            self.midi_out.message(&[0x80, ls, 0]).unwrap();
        }

        if stats.last_sent().is_none() || stats.last_sent().unwrap() != n.note {
            self.midi_out.message(&[0x90, n.note, n.velocity]).unwrap();
            stats.put_sent(&n);
        } else {
            stats.clear();
        }
    }
}

impl Drop for HoldingThru {
    fn drop(&mut self) {
        self.midi_out.message(&[0xB0, 0x7B, 0]).unwrap();
        println!("HoldingThru closed");
    }
}


fn index_of(substr: &str, input: &RtMidiIn) -> u32 {
    for port in 0..input.port_count().unwrap() {
        let name = input.port_name(port).unwrap();
        if name.to_lowercase().contains(&substr.to_lowercase()) {
            println!("found input port {} for {}", port, substr);
            return port;
        }
    }
    0
}

fn main() -> Result<(), RtMidiError> {

    let input = RtMidiIn::new(Default::default())?;
    let input_ports = input.port_count()?;
    println!("{} MIDI input sources", input_ports);
    for port in 0..input_ports {
        println!("Input {}: {}", port + 1, input.port_name(port)?);
    }
    let korg_port = index_of("USB", &input);

    input.open_port(korg_port, "RtMidi Input")?;

    let scale = Scale::from(48, &LYDIAN);
    let stats = Mutex::new(NoteStats::new());

    let hold_korg = HoldingThru::using_device("USB");
    let hold_d110 = HoldingThru::using_device("EDIROL");

    let oct_korg = RandomOctaveStage::to(4, -1, &hold_korg);
    let oct_d110 = RandomOctaveStage::to(2, -1, &hold_d110);

    let splitter = NoteSplitter::to(&oct_korg, &oct_d110);
    let mapper = NoteMapThru::to(&scale, &splitter);
    let sink = InputRegister::then(&mapper);

    let (midi_in_tx, midi_in_rx) = mpsc::channel();
    input.set_callback(|_timestamp, message| {
        if message[0] == 0x90 && message[2] != 0 {
            let n = Note::from_midi_message(&message);
            let mut s = stats.lock().unwrap();
            sink.receive(&n, &mut s);
            midi_in_tx.send(n.note).unwrap();
        }
    })?;

    input.ignore_types(true, true, true)?;

    thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        loop {
            match midi_in_rx.try_recv() {
                Ok(_) => {
                    println!("{:?}", midi_in_rx);

                    let res = client.post("http://localhost:7878")
                                .body("{}")
                                .send().unwrap();
                    println!("{:?}", res);
                },
                _ => {}
            }
        }
    });

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
