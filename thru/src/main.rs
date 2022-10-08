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


struct InputRegister {
    next: Rc<dyn MidiNoteSink>
}

impl MidiNoteSink for InputRegister {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        stats.put_received(&n);
        self.next.receive(&n, stats);
    }
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
    stats.put_sent(&n);
}

fn note_off(n: u8, midi_out: &RtMidiOut) {
    midi_out.message(&[0x80, n, 0]).unwrap();
}

// SimpleThru

struct ChordalThru {
    midi_out: Rc<RtMidiOut>
}


impl MidiNoteSink for ChordalThru {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        note_on(&n, &self.midi_out, stats);
    }
}

impl Drop for ChordalThru {
    fn drop(&mut self) {
        send_all_note_off(&self.midi_out);
        println!("SimpleThru closed");
    }
}


// HoldingThru

struct HoldingThru<> {
    midi_out: Rc<RtMidiOut>
}


impl MidiNoteSink for HoldingThru {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        if !stats.last_sent().is_none() {
            let ls = stats.last_sent().unwrap();
            note_off(ls, &self.midi_out);
            stats.put_cleared();
        }

        if stats.last_sent().is_none() || stats.last_sent().unwrap() != n.note {
            note_on(&n, &self.midi_out, stats);
        } else {
            stats.clear();
        }
    }
}

impl Drop for HoldingThru {
    fn drop(&mut self) {
        send_all_note_off(&self.midi_out);
        println!("HoldingThru closed");
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

    match &route.last().unwrap()[..] {
        "hold" => seq.push(Rc::new(HoldingThru { midi_out })),
        _ => seq.push(Rc::new(ChordalThru { midi_out }))
    }

    for r in route.into_iter().rev() {
        let next = Rc::clone(&seq[seq.len() - 1]);
        let scale = Rc::clone(&s);
        match &r[..] {
            "randomOctaveTop" => seq.push(Rc::new(RandomOctaveStage::to(4, 0, next))),
            "randomOctaveBass" => seq.push(Rc::new(RandomOctaveStage::to(2, -1, next))),
            "noteMap" => seq.push(Rc::new(NoteMap { next, scale })),
            "randomNoteMap" => seq.push(Rc::new(RandomNoteMap { next, scale })),
            "register" => seq.push(Rc::new(InputRegister { next })),
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
            vec!("register", "noteMap", "randomOctaveTop", "hold"),
            vec!("dropper", "register", "noteMap", "randomOctaveBass", "hold")
        ),
        (
            49,
            "aeolian",
            vec!("dropper", "register", "noteMap", "randomOctaveTop", "hold"),
            vec!("dropper", "register", "noteMap", "randomOctaveBass", "hold")
        ),
        (
            50,
            "aeolian",
            vec!("register", "randomNoteMap", "randomOctaveTop", "chord"),
            vec!("register", "randomNoteMap", "randomOctaveBass", "chord")
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
