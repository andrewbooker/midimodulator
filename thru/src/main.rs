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
type PlayedNote = (u8, u8);

struct NoteStats {
    received: [PlayedNote; NOTE_HISTORY]
}


impl NoteStats {
    fn new() -> NoteStats {
        NoteStats {
            received: [(0, 0); NOTE_HISTORY]
        }
    }

    fn last(&self) -> PlayedNote {
        self.received[NOTE_HISTORY - 1]
    }

    fn look_back(&self, b: u8) -> PlayedNote {
        self.received[NOTE_HISTORY - usize::from(b)]
    }

    fn sending_note_on(&mut self, n: u8, c: u8) {
        for i in 1..NOTE_HISTORY {
            self.received[i - 1] = self.received[i];
        }
        self.received[NOTE_HISTORY - 1] = (n, c);
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

impl RandomNoteDropper {
    fn should_play() -> bool {
        rand::random::<f64>() > 0.7
    }
}

impl MidiNoteSink for RandomNoteDropper {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        if Self::should_play() {
            self.next.receive(n, stats);
        }
    }
}


// NotifyingRandomNoteDropper

struct NotifyingRandomNoteDropper {
    next: Rc<dyn MidiNoteSink>
}

impl MidiNoteSink for NotifyingRandomNoteDropper {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        if RandomNoteDropper::should_play() {
            self.next.receive(n, stats);
        } else {
            thread::spawn(move || {
                post_cmd_to_modulator();
            });
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
    for c in 0..16 {
        midi_out.message(&[0xB0 | c, 0x7B, 0]).unwrap();
    }
}


// OutputStage

struct OutputStage {
    midi_out: Rc<RtMidiOut>,
    hold_length: u8,
    should_record: bool,
    channel_range: bool
}

impl OutputStage {
    fn channel(&self, stats: &NoteStats) -> u8 {
        if !self.channel_range {
            return 0;
        }
        return (stats.last().1 + 1) % 2
    }

    fn note_on(&self, n: &Note, stats: &mut NoteStats) {
        let c = self.channel(&stats);
        self.midi_out.message(&[0x90 | c, n.note, n.velocity]).unwrap();
        stats.sending_note_on(n.note, c);
        if self.should_record {
            post_cmd_to_recorder(object!{
                action: "on",
                note: n.note
            });
        }
    }

    fn note_off(&self, n: u8, channel: u8) {
        self.midi_out.message(&[0x80 | channel, n, 0]).unwrap();
        if self.should_record {
            post_cmd_to_recorder(object!{
                action: "off"
            });
        }
    }
}

impl MidiNoteSink for OutputStage {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        if self.hold_length == 0 {
            self.note_on(&n, stats);
            thread::sleep(Duration::from_millis(40));
            self.note_off(n.note, self.channel(&stats));
            return;
        }
    
        if self.hold_length == 1 {
            let l = stats.last();
            self.note_off(l.0, l.1);
            if n.note != l.0 {
                self.note_on(&n, stats);
            }
            return;
        }

        if n.note == stats.last().0 {
            return;
        }

        let prev = stats.look_back(self.hold_length);
        if prev.0 != 0 {
            self.note_off(prev.0, prev.1);
        }
        self.note_on(&n, stats);
    }
}

impl Drop for OutputStage {
    fn drop(&mut self) {
        send_all_note_off(&self.midi_out);
        println!("OutputStage closed");
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


const KORG_OUT: &str = "4i4o MIDI 3";
const D110_OUT: &str = "4i4o MIDI 4";
const MIDI_IN: &str = "4i4o MIDI 4";
const NUM_PARTS: usize = 2;


fn configure(route: &Vec<&str>, s: Rc<Scale>, midi_out: Rc<RtMidiOut>) -> Rc<dyn MidiNoteSink> {
    let mut seq = Vec::<Rc<dyn MidiNoteSink>>::new();

    let os: Vec<&str> = route.last().unwrap().split("_").collect();
    let hold_length: u8 = os[0].parse().unwrap();
    let should_record = os.len() > 1 && os[1] == "R";
    let channel_range = if hold_length < 3 { true } else { false };
    seq.push(Rc::new(OutputStage { midi_out, hold_length, should_record, channel_range }));

    for r in route.into_iter().rev() {
        let next = Rc::clone(&seq[seq.len() - 1]);
        let scale = Rc::clone(&s);
        match &r[..] {
            "randomOctaveTop" => seq.push(Rc::new(RandomOctaveStage::to(3, 0, next))),
            "randomOctaveBass" => seq.push(Rc::new(RandomOctaveStage::to(2, -1, next))),
            "noteMap" => seq.push(Rc::new(NoteMap { next, scale })),
            "randomNoteMap" => seq.push(Rc::new(RandomNoteMap { next, scale })),
            "dropper" => seq.push(Rc::new(RandomNoteDropper { next })),
            "notifyingDropper" => seq.push(Rc::new(NotifyingRandomNoteDropper { next })),
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
            vec!("noteMap", "randomOctaveTop", "1_R"),
            vec!("dropper", "noteMap", "randomOctaveBass", "1")
        ),
        (
            49,
            "aeolian",
            vec!("dropper", "noteMap", "randomOctaveTop", "3"),
            vec!("notifyingDropper", "noteMap", "randomOctaveBass", "2")
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
        Mutex::new(NoteStats::new()),
        Mutex::new(NoteStats::new())
    ];

    let korg_midi_out = Rc::new(find_output_from(KORG_OUT));
    let d110_midi_out = Rc::new(find_output_from(D110_OUT));

    let (tonic, mode, korg, d110) = &midi_input_routing()[1];
    let scale = Rc::new(Scale::from(*tonic, &modes[mode]));

    let parts: [Rc<dyn MidiNoteSink>; NUM_PARTS] = [
        configure(d110, Rc::clone(&scale), Rc::clone(&d110_midi_out)),
        configure(korg, Rc::clone(&scale), Rc::clone(&korg_midi_out))
    ];

    input.set_callback(|_timestamp, message| {
        if message[0] == 0x90 && message[2] != 0 {
            let n = Note::from_midi_message(&message);

            for i in 0..NUM_PARTS {
                let mut st = stats[i].lock().unwrap();
                parts[i].receive(&n, &mut st);
            }
        }
    })?;

    input.ignore_types(true, true, true)?;

    println!("Starting...");

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
