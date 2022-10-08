use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};
use json::{object, JsonValue};
use std::time::Duration;
use std::collections::HashMap;
use reqwest::StatusCode;
use std::rc::Rc;


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


const NOTE_HISTORY: usize = 8;

struct NoteStats {
    received: [u8; NOTE_HISTORY],
    record_on_play: bool
}


impl NoteStats {
    fn basic() -> NoteStats {
        NoteStats {
            received: [0, 0, 0, 0, 0, 0, 0, 0],
            record_on_play: false
        }
    }

    fn recording() -> NoteStats {
        NoteStats {
            received: [0, 0, 0, 0, 0, 0, 0, 0],
            record_on_play: true
        }
    }

    fn last(&self) -> u8 {
        self.received[NOTE_HISTORY - 1]
    }

    fn look_back(&self, b: u8) -> u8 {
        self.received[NOTE_HISTORY - usize::from(b)]
    }

    fn sending_note_on(&mut self, n: u8) {
        for i in 1..NOTE_HISTORY {
            self.received[i - 1] = self.received[i];
        }
        self.received[NOTE_HISTORY - 1] = n;
    }
}



trait MidiNoteSink {
    fn receive(&self, note: &Note, stats: &mut NoteStats);
}



// Scale

type Mode = [u8; 6];
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


// NoteMap

struct NoteMap {
    next: Rc<dyn MidiNoteSink>,
    scale: Rc<Scale>
}

impl MidiNoteSink for NoteMap {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let transposed = Note {
            note: self.scale.at(n.note),
            velocity: n.velocity
        };

        self.next.receive(&transposed, stats);
    }
}


// RandomNoteMap

struct RandomNoteMap {
    next: Rc<dyn MidiNoteSink>,
    scale: Rc<Scale>
}

impl MidiNoteSink for RandomNoteMap {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let r = rand::random::<f64>() * 8.0;
        let randomised = Note {
            note: self.scale.at(r.round() as u8),
            velocity: n.velocity
        };

        self.next.receive(&randomised, stats);
    }
}


// RandomNoteDropper

struct RandomNoteDropper {
    next: Rc<dyn MidiNoteSink>
}

impl MidiNoteSink for RandomNoteDropper {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let r = rand::random::<f64>();
        if r > 0.7 {
            self.next.receive(n, stats);
        }
    }
}


// RandomOctaveStage

struct RandomOctaveStage {
    octave_range: u8,
    base: i8,
    next: Rc<dyn MidiNoteSink>
}

impl RandomOctaveStage {
    pub fn to(octave_range: u8, base: i8, next: Rc<dyn MidiNoteSink>) -> RandomOctaveStage {
        RandomOctaveStage {
            octave_range,
            base,
            next
        }
    }
}

impl MidiNoteSink for RandomOctaveStage {
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


fn find_output_from(substr: &str) -> RtMidiOut {
    let m = RtMidiOut::new(Default::default()).unwrap();

    for port in 0..m.port_count().unwrap() {
        let name = m.port_name(port).unwrap();
        if name.to_lowercase().contains(&substr.to_lowercase()) {
            println!("found output port {} for {} ({})", port, substr, name);
            m.open_port(port, "HoldingThru out").unwrap();
        }
    }
    m
}


fn send_all_note_off(midi_out: &RtMidiOut) {
    midi_out.message(&[0xB0, 0x7B, 0]).unwrap();
}

fn note_on(n: &Note, midi_out: &RtMidiOut, stats: &mut NoteStats) {
    midi_out.message(&[0x90, n.note, n.velocity]).unwrap();
    stats.sending_note_on(n.note);
}

fn note_off(n: u8, midi_out: &RtMidiOut) {
    midi_out.message(&[0x80, n, 0]).unwrap();
}


// OutputStage

struct OutputStage {
    midi_out: Rc<RtMidiOut>,
    hold_length: u8
}


impl MidiNoteSink for OutputStage {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        if self.hold_length == 0 {
            note_on(&n, &self.midi_out, stats);
            thread::sleep(Duration::from_millis(40));
            note_off(n.note, &self.midi_out);
            return;
        }
    
        if self.hold_length == 1 {
            if n.note == stats.last() {
                note_off(n.note, &self.midi_out);
            } else {
                note_off(stats.last(), &self.midi_out);
                note_on(&n, &self.midi_out, stats);
            }
            return;
        }

        if n.note == stats.last() {
            println!("Ignoring {} same as last with hold length {}", n.note, self.hold_length);
            return;
        }

        let prev = stats.look_back(self.hold_length);
        if prev != 0 {
            note_off(prev, &self.midi_out);
        }
        note_on(&n, &self.midi_out, stats);
    }
}

impl Drop for OutputStage {
    fn drop(&mut self) {
        send_all_note_off(&self.midi_out);
        println!("OutputStage closed");
    }
}



// DeadEnd

struct DeadEnd;

impl MidiNoteSink for DeadEnd {
    fn receive(&self, _: &Note, _: &mut NoteStats) {}
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


const KORG_OUT: &str = "4i4o MIDI 3";
const D110_OUT: &str = "4i4o MIDI 4";
const MIDI_IN: &str = "4i4o MIDI 4";
const NUM_PARTS: usize = 2;


fn configure(route: &Vec<&str>, s: Rc<Scale>, midi_out: Rc<RtMidiOut>) -> Rc<dyn MidiNoteSink> {
    let mut seq = Vec::<Rc<dyn MidiNoteSink>>::new();

    let hold_length = 0;
    seq.push(Rc::new(OutputStage { midi_out, hold_length }));

    for r in route.into_iter().rev() {
        let next = Rc::clone(&seq[seq.len() - 1]);
        let scale = Rc::clone(&s);
        match &r[..] {
            "randomOctaveTop" => seq.push(Rc::new(RandomOctaveStage::to(4, 0, next))),
            "randomOctaveBass" => seq.push(Rc::new(RandomOctaveStage::to(2, -1, next))),
            "noteMap" => seq.push(Rc::new(NoteMap { next, scale })),
            "randomNoteMap" => seq.push(Rc::new(RandomNoteMap { next, scale })),
            "dropper" => seq.push(Rc::new(RandomNoteDropper { next })),
            _ => {}
        }
    }
    Rc::clone(&seq.last().unwrap())
}

type TonicModeKorgD110 = (u8, &'static str, Vec<&'static str>, Vec<&'static str>);
fn midi_input_routing() -> [TonicModeKorgD110; 3] {
    [
        (
            48,
            "lydian",
            vec!("noteMap", "randomOctaveTop", "1"),
            vec!("dropper", "noteMap", "randomOctaveBass", "1")
        ),
        (
            49,
            "aeolian",
            vec!("dropper", "noteMap", "randomOctaveTop", "1"),
            vec!("dropper", "noteMap", "randomOctaveBass", "1")
        ),
        (
            50,
            "aeolian",
            vec!("randomNoteMap", "randomOctaveTop", "3"),
            vec!("randomNoteMap", "randomOctaveTop", "3")
        )
    ]
}


fn main() -> Result<(), RtMidiError> {

    let modes: HashMap<&str, Mode> = HashMap::from([
        ("aeolian", [2, 1, 2, 2, 1, 2]),
        ("lydian", [2, 2, 2, 1, 2, 2])
    ]);

    let input = RtMidiIn::new(Default::default())?;
    let input_ports = input.port_count()?;
    println!("{} MIDI input sources", input_ports);
    for port in 0..input_ports {
        println!("Input {}: {}", port + 1, input.port_name(port)?);
    }
    let input_port = index_of(MIDI_IN, &input);
    input.open_port(input_port, "RtMidi Input")?;

    let stats: [Mutex<NoteStats>; NUM_PARTS] = [
        Mutex::new(NoteStats::basic()),
        Mutex::new(NoteStats::recording())
    ];

    let korg_midi_out = Rc::new(find_output_from(KORG_OUT));
    let d110_midi_out = Rc::new(find_output_from(D110_OUT));

    let (tonic, mode, korg, d110) = &midi_input_routing()[2];
    let scale = Rc::new(Scale::from(*tonic, &modes[mode]));

    let parts: [Rc<dyn MidiNoteSink>; NUM_PARTS] = [
        configure(d110, Rc::clone(&scale), Rc::clone(&d110_midi_out)),
        Rc::new(DeadEnd {}) //configure(korg, Rc::clone(&scale), Rc::clone(&korg_midi_out))
    ];

    let (midi_in_tx, midi_in_rx) = mpsc::channel();
    input.set_callback(|_timestamp, message| {
        if message[0] == 0x90 && message[2] != 0 {
            let n = Note::from_midi_message(&message);

            for i in 0..NUM_PARTS {
                let mut st = stats[i].lock().unwrap();
                parts[i].receive(&n, &mut st);
            }

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
    let (cmd_note_off_tx, cmd_note_off_rx) = mpsc::channel();
    thread::spawn(move || {
        let g = getch::Getch::new();
        loop {
            let c: u8 = g.getch().unwrap();
            match c as char {
                'q' => {
                    cmd_stop_tx.send(()).unwrap();
                    break;
                },
                'o' => {
                    cmd_note_off_tx.send(()).unwrap();
                }
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
            _ => thread::sleep(Duration::from_millis(50))
        }
        match cmd_note_off_rx.try_recv() {
            Ok(_) => {
                post_cmd_to_recorder(object!{
                    action: "off"
                });
                send_all_note_off(&korg_midi_out);
                send_all_note_off(&d110_midi_out);
            },
            _ => thread::sleep(Duration::from_millis(50))
        }

    }

    Ok(())
}
