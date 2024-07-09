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
use rtmidi::RtMidiOut;


pub fn configure(route: &Vec<&str>, s: Rc<Scale>, midi_out: Rc<RtMidiOut>) -> Rc<dyn MidiNoteSink> {
    let mut seq = Vec::<Rc<dyn MidiNoteSink>>::new();

    let os: Vec<&str> = route.last().unwrap().split("_").collect();
    let hold_length: u8 = os[0].parse().unwrap();
    let should_record = os.len() > 1 && os[1] == "R";
    let channel_range = if hold_length < 3 { 3 } else { 0 };
    seq.push(Rc::new(OutputStage { midi_out, hold_length, should_record, channel_range }));

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

