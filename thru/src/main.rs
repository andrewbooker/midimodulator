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


use crate::outputstage::{
    send_all_note_off,
    OutputStage
};

use crate::configure::configure;

use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::thread;
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};
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
const NUM_PARTS: usize = 1;


fn main() -> Result<(), RtMidiError> {
    let modes: HashMap<&str, Mode> = HashMap::from([
        ("minorpentatonic", [3, 2, 2, 3, 2, 3]),
        ("aeolian", [2, 1, 2, 2, 1, 2]),
        ("lydian", [2, 2, 2, 1, 2, 2]),
        ("mixolydian", [2, 2, 1, 2, 2, 1])
    ]);

    let args: Vec<_> = env::args().collect();
    let tonic = if args.len() > 1 { args[1].parse::<u8>().unwrap() } else { 65 };
    let mode = if args.len() > 2 { args[2].as_str() } else { "minorpentatonic" };

    println!("Playing {} {}", tonic, mode);

    let input = RtMidiIn::new(Default::default())?;
    let input_port = index_of("4i4o MIDI 4", &input);
    input.open_port(input_port, "RtMidi Input")?;

    let stats: [Mutex<NoteStats>; NUM_PARTS] = [
        Mutex::new(NoteStats::new())
    ];

    let korg_midi_out = Arc::new(find_output_from(KORG_OUT));
    let korg = vec!("randomNoteMap", "randomOctaveMid");

    let scale = Rc::new(Scale::from(tonic, &modes[mode]));
    let selector = Arc::new(RwLock::new(NoteSelector::new(Rc::clone(&scale))));
    let korg_output_stage = Rc::new(OutputStage { midi_out: Arc::clone(&korg_midi_out), should_record: false, channel_range: 0 });
    let parts: [Rc<dyn MidiNoteSink>; NUM_PARTS] = [
        configure(&korg, Rc::clone(&scale), Arc::clone(&selector), Rc::clone(&korg_output_stage))
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
    let (cmd_hold_on_tx, cmd_hold_on_rx) = mpsc::channel();
    let (cmd_hold_off_tx, cmd_hold_off_rx) = mpsc::channel();
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
                'n' => {
                    cmd_note_test_tx.send(()).unwrap();
                },
                'c' | 't' | 'l' | 'r' | 'u' | 'd' => {
                    cmd_note_tx.send(c).unwrap();
                },
                'h' => {
                    cmd_hold_on_tx.send(()).unwrap();
                },
                'f' => {
                    cmd_hold_off_tx.send(()).unwrap();
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
                send_all_note_off(&korg_midi_out);
                println!("All notes off");
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
        match cmd_note_rx.try_recv() {
            Ok(n) => {
                let mut sel = selector.write().unwrap();
                sel.set_strategy_from(n);
            },
            _ => thread::sleep(Duration::from_millis(50))
        }
        match cmd_hold_on_rx.try_recv() {
            Ok(_) => {
                for i in 0..NUM_PARTS {
                    let mut st = stats[i].lock().unwrap();
                    st.hold_on();
                }
                println!("Hold on");
            },
            _ => thread::sleep(Duration::from_millis(50))
        }
        match cmd_hold_off_rx.try_recv() {
            Ok(_) => {
                send_all_note_off(&korg_midi_out);
                for i in 0..NUM_PARTS {
                    let mut st = stats[i].lock().unwrap();
                    st.hold_off();
                }
                println!("Hold off");
            },
            _ => thread::sleep(Duration::from_millis(50))
        }
    }
    Ok(())
}
