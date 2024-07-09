use crate::note::{
    Note,
    NoteStats
};

use crate::notesink::{
    MidiNoteSink
};

use crate::interop::{
    post_cmd_to_recorder
};

use std::rc::Rc;
use rtmidi::RtMidiOut;
use json::object;
use std::thread;
use std::time::Duration;


pub fn send_all_note_off(midi_out: &RtMidiOut) {
    for c in 0..16 {
        midi_out.message(&[0xB0 | c, 0x7B, 0]).unwrap();
    }
}


// OutputStage

pub struct OutputStage {
    pub midi_out: Rc<RtMidiOut>,
    pub hold_length: u8,
    pub should_record: bool,
    pub channel_range: u8
}

impl OutputStage {
    fn channel(&self, stats: &NoteStats) -> u8 {
        if self.channel_range == 0 {
            return 0;
        }
        return (stats.last().1 + 1) % self.channel_range
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

