

#[path = "../../lib/midi.rs"] mod midi;

use crate::midi::{MidiMessage, MidiOut, MidiOutDevices, MidiIn, MidiInDevices};
use std::{
    thread,
    time::Duration,
    sync::mpsc
};

const CHANNEL: u8 = 0;

fn note_test(midi_out: &mut MidiOut, note: u8) {
    let on = MidiMessage::note_on(note, CHANNEL);
    let off = MidiMessage::note_off(note, CHANNEL);

    midi_out.send(&on);
    thread::sleep(Duration::from_millis(325));
    midi_out.send(&off);
    thread::sleep(Duration::from_millis(125));
}

fn note_read(midi_in: &mut MidiIn) {
    midi_in.read();
}


fn main() {
    MidiOutDevices::list();
    MidiInDevices::list();
    
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut midi_in = MidiIn::using_device(3);
        loop {
            note_read(&mut midi_in);
        }
    });
    
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
}
