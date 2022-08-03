

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




struct HoldingThru {
    midi_out: MidiOut,
    last_note: u8
}

impl MidiCallback for HoldingThru {
    fn receive(&mut self, msg: &MidiMessage) {
        let channel = msg.status & 0xF;
        let instruction = msg.status & 0xF0;
        if msg.data2 > 0 {
            if self.last_note != 0 {
                let off = MidiMessage::note_off(self.last_note, CHANNEL);
                self.midi_out.send(&off);
            }
            let on = MidiMessage::note_on(msg.data1, CHANNEL);
            self.midi_out.send(&on);
            self.last_note = msg.data1;
        }
    }
}

impl Drop for HoldingThru {
    fn drop(&mut self) {
        if self.last_note != 0 {
            let off = MidiMessage::note_off(self.last_note, CHANNEL);
            self.midi_out.send(&off);
        }
        println!("HoldingThru closed");
    }
}


fn main() {
    MidiOutDevices::list();
    MidiInDevices::list();
    
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();
    let (midi_stop_tx, midi_stop_rx) = mpsc::channel();

    let midi_loop_handle = thread::spawn(move || {
        let mut midi_in = MidiIn::using_device(3);
        let mut thru = HoldingThru {
            midi_out: MidiOut::using_device(2),
            last_note: 0
        };
        loop {
            midi_in.read(&mut thru);
            match midi_stop_rx.try_recv() {
                Ok(_) => {
                    break;
                },
                _ => {}
            }
        }
    });
    
    thread::spawn(move || {
        let g = getch::Getch::new();
        loop {
            let c: u8 = g.getch().unwrap();
            match c as char {
                'q' => {
                    cmd_stop_tx.send(()).unwrap();
                    midi_stop_tx.send(()).unwrap();
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

    midi_loop_handle.join();
}
