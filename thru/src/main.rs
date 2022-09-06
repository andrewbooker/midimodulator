use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};
use json::{object, JsonValue};
use std::time::Duration;
use reqwest::StatusCode;
use std::error::Error;



fn post_cmd(port: u16, data: JsonValue) {
    let client = reqwest::blocking::Client::new();
    match client.post(format!("http://localhost:{}", port))
                    .header("Content-type", "application/json")
                    .body(data.dump())
                    .send() {
        Err(e) => println!("{:?}", e),
        Ok(res) => {
            if res.status() != StatusCode::OK {
                println!("{:?} {:?}", data, res.status());
            }
        }
    }
}


fn post_cmd_to_recorder(data: JsonValue) {
    post_cmd(9009, data);
}

fn post_cmd_to_modulator() {
    post_cmd(7878, object!{});
}


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
    sent: u8,
    record_on_play: bool
}





impl NoteStats {
    fn basic() -> NoteStats {
        NoteStats {
            received: 0,
            sent: 0,
            record_on_play: false
        }
    }

    fn recording() -> NoteStats {
        NoteStats {
            received: 0,
            sent: 0,
            record_on_play: true
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
            if self.record_on_play {
                post_cmd_to_recorder(object!{
                    action: "on",
                    note: n.note
                });
            }
        } else {
            self.sent = 0;
        }
    }

    fn put_cleared(&self) {
        if self.record_on_play {
            post_cmd_to_recorder(object!{
                action: "off"
            });
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


// RandomNoteDropper

struct RandomNoteDropper<'a, S: MidiNoteSink> {
    next: &'a S
}

impl <'a, S: MidiNoteSink>MidiNoteSink for RandomNoteDropper<'a, S> {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let r = rand::random::<f64>();
        if r > 0.7 {
            self.next.receive(n, stats);
        }
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
            stats.put_cleared();
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
    let input_port = index_of("USB", &input);
    input.open_port(input_port, "RtMidi Input")?;

    let scale = Scale::from(48, &LYDIAN);

    let stats_d110 = Mutex::new(NoteStats::basic());
    let stats_korg = Mutex::new(NoteStats::recording());

    let hold_korg = HoldingThru::using_device("USB");
    let oct_korg = RandomOctaveStage::to(4, 0, &hold_korg);
    let mapper_korg = NoteMapThru::to(&scale, &oct_korg);
    let register_korg = InputRegister::then(&mapper_korg);

    let hold_d110 = HoldingThru::using_device("EDIROL");
    let oct_d110 = RandomOctaveStage::to(2, -1, &hold_d110);
    let mapper_d110 = NoteMapThru::to(&scale, &oct_d110);
    let register_d110 = InputRegister::then(&mapper_d110);
    let dropper = RandomNoteDropper { next: &register_d110 };

    let (midi_in_tx, midi_in_rx) = mpsc::channel();
    input.set_callback(|_timestamp, message| {
        if message[0] == 0x90 && message[2] != 0 {
            let n = Note::from_midi_message(&message);

            let mut s_d110 = stats_d110.lock().unwrap();
            let mut s_korg = stats_korg.lock().unwrap();
            dropper.receive(&n, &mut s_d110);
            register_korg.receive(&n, &mut s_korg);
            midi_in_tx.send(n.note).unwrap();

        }
    })?;

    input.ignore_types(true, true, true)?;

    println!("Starting...");

    thread::spawn(move || {
        loop {
            match midi_in_rx.try_recv() {
                Ok(_) => post_cmd_to_modulator(),
                _ => thread::sleep(Duration::from_millis(100))
            }
        }
    });

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
            _ => thread::sleep(Duration::from_millis(100))
        }
    }
    
    Ok(())
}
