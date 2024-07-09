mod note;
mod notesink;
mod interop;
mod outputstage;

use crate::note::{
    Note,
    NoteStats,
    Mode,
    Scale
};

use crate::notesink::{
    MidiNoteSink,
    NoteMap,
    RandomNoteMap,
    RandomNoteDropper,
    NotifyingRandomNoteDropper,
    RandomOctaveStage
};

use crate::interop::{
    post_cmd_to_recorder
};

use crate::outputstage::{
    OutputStage,
    send_all_note_off
};

use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};
use json::object;
use std::time::Duration;
use std::collections::HashMap;
use std::rc::Rc;



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
const NUM_PARTS: usize = 2;


fn configure(route: &Vec<&str>, s: Rc<Scale>, midi_out: Rc<RtMidiOut>) -> Rc<dyn MidiNoteSink> {
    let mut seq = Vec::<Rc<dyn MidiNoteSink>>::new();

    let os: Vec<&str> = route.last().unwrap().split("_").collect();
    let hold_length: u8 = os[0].parse().unwrap();
    let should_record = os.len() > 1 && os[1] == "R";
    let channel_range = if hold_length < 3 { 3 } else { 0 };
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
    let input_port = index_of("USB MIDI Interface", &input);
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
    let (cmd_note_test_tx, cmd_note_test_rx) = mpsc::channel();
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
                },
                't' => {
                    cmd_note_test_tx.send(()).unwrap();
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
        match cmd_note_test_rx.try_recv() {
            Ok(_) => {
                let c = 0;
                korg_midi_out.message(&[0x90 | c, 60, 99]).unwrap();
            },
            _ => thread::sleep(Duration::from_millis(50))
        }
    }
    Ok(())
}
