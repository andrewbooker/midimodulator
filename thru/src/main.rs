mod note;
mod notesink;
mod interop;
mod outputstage;
mod configure;

use crate::note::{
    Note,
    NoteStats,
    Mode,
    Scale
};

use crate::notesink::{
    MidiNoteSink
};

use crate::interop::{
    post_cmd_to_recorder
};

use crate::outputstage::{
    send_all_note_off
};

use crate::configure::configure;

use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};
use json::object;
use std::time::Duration;
use std::collections::HashMap;
use std::rc::Rc;
use std::env;



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



type Routing = Vec<&'static str>;
fn routing_korg() -> [Routing; 3] {
    [
        vec!("noteMap", "randomOctaveTop", "1_R"),
        vec!("dropper", "noteMap", "randomOctaveTop", "3"),
        vec!("randomNoteMap", "randomOctaveTop", "3")
    ]
}

fn routing_d110() -> [Routing; 3] {
    [
        vec!("dropper", "noteMap", "randomOctaveBass", "1"),
        vec!("notifyingDropper", "noteMap", "randomOctaveBass", "2"),
        vec!("randomNoteMap", "randomOctaveTop", "3")
    ]
}


fn main() -> Result<(), RtMidiError> {

    let modes: HashMap<&str, Mode> = HashMap::from([
        ("aeolian", [2, 1, 2, 2, 1, 2]),
        ("lydian", [2, 2, 2, 1, 2, 2])
    ]);

    let args: Vec<_> = env::args().collect();
    let tonic = if args.len() > 1 { args[1].parse::<u8>().unwrap() } else { 60 };
    let mode = if args.len() > 2 { args[2].as_str() } else { "aeolian" };

    println!("Playing {} {}", tonic, mode);

    let input = RtMidiIn::new(Default::default())?;
    let input_port = index_of("USB MIDI Interface", &input);
    input.open_port(input_port, "RtMidi Input")?;

    let stats: [Mutex<NoteStats>; NUM_PARTS] = [
        Mutex::new(NoteStats::new()),
        Mutex::new(NoteStats::new())
    ];

    let korg_midi_out = Rc::new(find_output_from(KORG_OUT));
    let d110_midi_out = Rc::new(find_output_from(D110_OUT));

    let korg = &routing_korg()[1];
    let d110 = &routing_d110()[1];
    let scale = Rc::new(Scale::from(tonic, &modes[mode]));

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
