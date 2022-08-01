

#[path = "../../lib/midi.rs"] mod midi;

use crate::midi::{MidiMessage, MidiOut, MidiOutDevices};
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


fn main() {
    MidiOutDevices::list();
    
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
    
    let mut midi_out = MidiOut::using_device(2);
    let mut n = 0;
    loop {
        note_test(&mut midi_out, 40 + (2 * n));
        n += 1;
        if n > 20 {
            n = 0;
        }
        match cmd_stop_rx.try_recv() {
            Ok(_) => {
                println!("stopping...");
                break;
            },
            _ => {}
        }
    }
}
