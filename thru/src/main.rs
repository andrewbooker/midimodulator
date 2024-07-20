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
    MidiNoteSink,
    NoteSelector
};

use crate::interop::{
    post_cmd_to_recorder
};

use crate::outputstage::{
    send_all_note_off,
    OutputStage
};

use crate::configure::configure;

use std::sync::{Arc, mpsc, Mutex, RwLock};
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
    println!("{} MIDI input ports", input.port_count().unwrap());
    println!("finding {}", substr);
    for port in 0..input.port_count().unwrap() {
        println!("trying {}", port);
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
        vec!("noteMap", "randomOctaveTop"),
        vec!("dropper", "noteMap", "randomOctaveTop"),
        vec!("randomNoteMap", "randomOctaveTop")
    ]
}

fn routing_d110() -> [Routing; 3] {
    [
        vec!("dropper", "noteMap", "randomOctaveBass"),
        vec!("notifyingDropper", "noteMap", "randomOctaveBass"),
        vec!("notifyingDropper", "randomNoteMap", "randomOctaveMid")
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

    let korg_midi_out = Arc::new(find_output_from(KORG_OUT));
    let d110_midi_out = Arc::new(find_output_from(D110_OUT));

    let korg = &routing_korg()[2];
    let d110 = &routing_d110()[2];
    let scale = Rc::new(Scale::from(tonic, &modes[mode]));

    let selector = Arc::new(RwLock::new(NoteSelector::new(Rc::clone(&scale))));

    let d110_output_stage = Rc::new(OutputStage { midi_out: Arc::clone(&d110_midi_out), hold_length: 1, should_record: false, channel_range: 0 });
    let korg_output_stage = Rc::new(OutputStage { midi_out: Arc::clone(&korg_midi_out), hold_length: 0, should_record: false, channel_range: 0 });

    let parts: [Rc<dyn MidiNoteSink>; NUM_PARTS] = [
        configure(d110, Rc::clone(&scale), Arc::clone(&selector), Rc::clone(&d110_output_stage)),
        configure(korg, Rc::clone(&scale), Arc::clone(&selector), Rc::clone(&korg_output_stage))
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
    let (cmd_note_tx, cmd_note_rx) = mpsc::channel();
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
                'l' | 'r' | 'c' => {
                    cmd_note_tx.send(c).unwrap();
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
                d110_midi_out.message(&[0x90 | c, 60, 99]).unwrap();
            },
            _ => thread::sleep(Duration::from_millis(50))
        }
        match cmd_note_rx.try_recv() {
            Ok(n) => {
                let mut sel = selector.write().unwrap();
                sel.set_strategy_from(n);
            },
            _ => thread::sleep(Duration::from_millis(50))
        }
    }
    Ok(())
}
