
use crate::note::{
    Note,
    NoteStats,
    Scale
};

use crate::interop::{
    post_cmd_to_modulator
};

use std::rc::Rc;
use std::sync::{Arc, RwLock};
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


// NoteSelector

pub struct NoteSelector {
    strategy: u8,
    scale: Rc<Scale>
}

impl NoteSelector { // possibly split out a trait for the mutabiity bit as it is only required in the main function and nowhere in here
    pub fn new(scale: Rc<Scale>) -> Self {
        Self { strategy: b't', scale }
    }

    fn next(&self, stats: &NoteStats) -> u8 {
        match self.strategy as char {
            'l' => {
                let mut last_note = stats.last().0;
                if last_note > (self.scale.notes[0] + 12) {
                    last_note -= 24;
                } else if last_note < (self.scale.notes[0] - 12) {
                    last_note += 24;
                }
                last_note
            },
            'r' => {
                let r = rand::random::<f64>() * 8.0;
                self.scale.at(r.round() as u8)
            },
            'u' => {
                self.scale.at(self.scale.ordinal_of(stats.last().0) + 1)
            },
            'd' => {
                let last_ordinal = self.scale.ordinal_of(stats.last().0);
                self.scale.at(if last_ordinal == 0 { 7 } else { last_ordinal - 1})
            },
            _ => self.scale.notes[0]
        }
    }

    pub fn set_strategy_from(&mut self, s: u8) {
        self.strategy = s;
        match self.strategy as char {
            'l' => println!("Playing last note"),
            'r' => println!("Playing random note"),
            'u' => println!("Cycling up scale"),
            'd' => println!("Cycling down scale"),
            _ => println!("Playing tonic")
        }
    }
}


// RandomNoteMap

pub struct RandomNoteMap {
    pub next: Rc<dyn MidiNoteSink>,
    selector: Arc<RwLock<NoteSelector>>
}

impl RandomNoteMap {
    pub fn create_from(next: Rc<dyn MidiNoteSink>, selector: Arc<RwLock<NoteSelector>>) -> Self {
        Self { next, selector }
    }
}

impl MidiNoteSink for RandomNoteMap {
    fn receive(&self, n: &Note, stats: &mut NoteStats) {
        let next = Note {
            note: self.selector.read().unwrap().next(&stats),
            velocity: n.velocity
        };

        self.next.receive(&next, stats);
    }
}


// RandomNoteDropper

pub struct RandomNoteDropper {
    pub next: Rc<dyn MidiNoteSink>
}

impl RandomNoteDropper {
    pub fn should_play() -> bool {
        rand::random::<f64>() > 0.4
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
        let millis_since_last_dropped = stats.last_dropped.2.elapsed().as_millis();
        if RandomNoteDropper::should_play() && millis_since_last_dropped > 500u128 {
            self.next.receive(n, stats);
        } else {
            thread::spawn(move || {
                post_cmd_to_modulator(note);
            });
            stats.drop(note);
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
    pub fn to(octave_range: u8, base: i8, next: Rc<dyn MidiNoteSink>) -> Self {
        Self {
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
