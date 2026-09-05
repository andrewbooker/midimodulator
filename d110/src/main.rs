extern crate libc;

mod midi;
mod d110;
mod utils;
mod modulation;


use crate::d110::{
    init_d110,
    init_timbre,
    set_up_tone,
    PARTIAL_SPEC,
    D110SysEx
};



use crate::modulation::{
    SysExComposer,
    PairedUpdater,
    StepInterval,
    SweepState,
    Selector
};


use crate::midi::{MidiMessage, MidiOut, MidiOutDevices};
use std::{
    thread,
    time::{Duration, Instant},
    sync::{mpsc, mpsc::{Sender, Receiver}},
    io::{prelude::*, BufReader},
    net::TcpListener,
    collections::HashMap
};
use rand::prelude::SliceRandom;


struct DummySelector;

impl DummySelector {
    fn new() -> DummySelector {
        DummySelector {}
    }
}

impl Selector for DummySelector {
    fn next1(&mut self) {}
    fn next2(&mut self) {}

    fn val(&self, _: u8) -> u16 { 0 }
}


struct TimeBasedInterval {
    start: Instant
}

impl TimeBasedInterval {
    fn new() -> TimeBasedInterval {
        TimeBasedInterval {
            start: Instant::now()
        }
    }
}

impl StepInterval for TimeBasedInterval {
    fn interval(&self) -> f32 {
        self.start.elapsed().as_millis() as f32
    }
}

struct FixedEquivalentMillisInterval {
    int: u32
}

impl FixedEquivalentMillisInterval {
    fn new(int: u32) -> FixedEquivalentMillisInterval {
        FixedEquivalentMillisInterval {
            int
        }
    }
}

impl StepInterval for FixedEquivalentMillisInterval {
    fn interval(&self) -> f32 {
        self.int as f32
    }
}



const NUM_D110_PARTS: usize = 3;

fn update_d110(updater: &mut PairedUpdater, d110_midi_out: &mut MidiOut) {
    let mut dummy_1 = DummySelector::new();
    let mut dummy_2 = DummySelector::new();

    let tones: [& mut D110SysEx; NUM_D110_PARTS] = [
        &mut set_up_tone(1),
        &mut set_up_tone(2),
        &mut set_up_tone(3)
    ];

    let prefixes = ["A_1", "B_3", "C_2", "D_4"];
    for t in 0..NUM_D110_PARTS {
        for p in prefixes {
            updater.update(tones[t], &mut dummy_1, &mut dummy_2, &PARTIAL_SPEC, Some(&*format!("tone{}_partial{}", t + 1, p)));
        }
    }

    updater.sweep_alternator();

    for t in 0..NUM_D110_PARTS {
        let v = tones[t].to_send();
        d110_midi_out.send_sys_ex(&v);
    }
}



fn start(d110_number: i32) {
    let mut d110_midi_out = MidiOut::using_device(d110_number);
    let d110_init = init_d110();
    d110_midi_out.send_sys_ex(&d110_init.to_send());
    for t in 1..9 {
        println!("sending timbre {}", t);
        d110_midi_out.send_sys_ex(&init_timbre(t).to_send());
    }
    for t in 1..9 {
        println!("intitialising part {}", t);
        d110_midi_out.send_sys_ex(&set_up_tone(t).to_send());
    }
    println!("D110 init sent");

    let count: u32 = 1;
    let interval = FixedEquivalentMillisInterval::new(1000 * count);
    let mut updater = PairedUpdater::new(&interval);
    update_d110(&mut updater, &mut d110_midi_out);
}


fn main() {
    let d110_number = MidiOutDevices::index_of("USB MIDI").unwrap();
    println!("D110 port {}", d110_number);
    start(d110_number);
}
