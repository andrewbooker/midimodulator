use crate::note::{
    Scale
};

use crate::notesink::{
    MidiNoteSink,
    RandomOctaveStage,
    NoteMap,
    RandomNoteMap,
    RandomNoteDropper,
    NotifyingRandomNoteDropper
};

use crate::outputstage::{
    OutputStage
};

use std::rc::Rc;


pub fn configure(route: &Vec<&str>, s: Rc<Scale>, out: Rc<OutputStage>) -> Rc<dyn MidiNoteSink> {
    let mut seq = Vec::<Rc<dyn MidiNoteSink>>::new();
    seq.push(out);

    for r in route.into_iter().rev() {
        let next = Rc::clone(&seq[seq.len() - 1]);
        let scale = Rc::clone(&s);
        match &r[..] {
            "randomOctaveTop" => seq.push(Rc::new(RandomOctaveStage::to(3, 0, next))),
            "randomOctaveBass" => seq.push(Rc::new(RandomOctaveStage::to(2, -1, next))),
            "noteMap" => seq.push(Rc::new(NoteMap { next, scale })),
            "randomNoteMap" => seq.push(Rc::new(RandomNoteMap { next, scale })),
            "dropper" => seq.push(Rc::new(RandomNoteDropper { next })),
            "notifyingDropper" => seq.push(Rc::new(NotifyingRandomNoteDropper { next })),
            _ => {}
        }
    }
    Rc::clone(&seq.last().unwrap())
}

