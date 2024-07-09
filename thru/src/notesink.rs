
use crate::note::{
    Note,
    NoteStats,
    Scale
};

use crate::interop::{
    post_cmd_to_modulator
};

use std::rc::Rc;
use std::thread;

pub trait MidiNoteSink {
    fn receive(&self, note: &Note, stats: &mut NoteStats);
}


// NoteMap

pub struct NoteMap {
    pub next: Rc<dyn MidiNoteSink>,
    pub scale: Rc<Scale>
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

pub struct RandomNoteMap {
    pub next: Rc<dyn MidiNoteSink>,
    pub scale: Rc<Scale>
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

pub struct RandomNoteDropper {
    pub next: Rc<dyn MidiNoteSink>
}

impl RandomNoteDropper {
    pub fn should_play() -> bool {
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

pub struct NotifyingRandomNoteDropper {
    pub next: Rc<dyn MidiNoteSink>
}

impl MidiNoteSink for NotifyingRandomNoteDropper {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let note = n.note;
        if RandomNoteDropper::should_play() {
            self.next.receive(n, stats);
        } else {
            thread::spawn(move || {
                post_cmd_to_modulator(note);
            });
        }
    }
}


// RandomOctaveStage

pub struct RandomOctaveStage {
    pub octave_range: u8,
    pub base: i8,
    pub next: Rc<dyn MidiNoteSink>
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
