

#[path = "../../lib/midi.rs"] mod midi;

use crate::midi::{MidiMessage, MidiOut, MidiOutDevices, MidiIn, MidiInDevices, MidiCallback};
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




struct SimpleThru {
    midi_out: MidiOut
}

impl MidiCallback for SimpleThru {
    fn receive(&mut self, msg: &MidiMessage) {
        let channel = msg.status & 0xF;
        let instruction = msg.status & 0xF0;
        if msg.data2 > 0 {
            println!("channel: {}, instruction: 0x{:x}, note: {}, velocity: {}", channel + 1, instruction, msg.data1, msg.data2);
            let on = MidiMessage::note_on(msg.data1, CHANNEL);
            let off = MidiMessage::note_off(msg.data1, CHANNEL);
            self.midi_out.send(&on);
            thread::sleep(Duration::from_millis(1));
            self.midi_out.send(&off);
        }
    }
}


fn main() {
    MidiOutDevices::list();
    MidiInDevices::list();
    
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut midi_in = MidiIn::using_device(3);
        let mut thru = SimpleThru {
            midi_out: MidiOut::using_device(2)
        };
        loop {
            midi_in.read(&mut thru);
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
