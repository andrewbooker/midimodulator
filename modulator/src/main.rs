extern crate portmidi as midi;

use std::{
    f32,
    thread,
    time::{Duration, Instant}
};


struct ModulationProfile {
    freq_hz: f32,
    min_val: i32,
    max_val: i32,

    previous_val: i32,
    current_val: i32
}

impl ModulationProfile {
    fn new(f_hz: f32, min_v: i32, max_v: i32) -> ModulationProfile {
        ModulationProfile {
            freq_hz: f_hz,
            min_val: min_v,
            max_val: max_v,
            current_val: max_v,
            previous_val: max_v
        }
    }

    fn update(&mut self, since_start: &Instant) {
        let dt = since_start.elapsed().as_millis() as f32;
        let ang_freq = self.freq_hz * 2.0 * f32::consts::PI as f32;

        self.previous_val = self.current_val;
        let val = self.min_val as f32 + ((self.max_val - self.min_val) as f32 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()));
        self.current_val = val.round() as i32;
    }
}


fn print_devices(pm: &midi::PortMidi) {
    for device in pm.devices().unwrap() {
        println!("{}", device);
    }
}


use midi::MidiMessage;
fn test_midi(mut out_port: midi::OutputPort) {
    let channel: u8 = 0;
    let melody: [(u8, u32); 7] = [
        (60, 1),
        (60, 2),
        (67, 1),
        (67, 2),
        (69, 1),
        (69, 2),
        (67, 3)
    ];

    for &(note, dur) in melody.iter() {
        let note_on = MidiMessage {
            status: 0x90 + channel,
            data1: note,
            data2: 100,
            data3: 0
        };
        println!("{}", note_on);
        out_port.write_message(note_on);
        thread::sleep(Duration::from_millis(dur as u64 * 400));

        let note_off = MidiMessage {
            status: 0x80 + channel,
            data1: note,
            data2: 0,
            data3: 0
        };
        println!("{}", note_off);
        out_port.write_message(note_off);
    }
}

fn main() {
    let start = Instant::now();
    let mut mp = ModulationProfile::new(0.05, -50, 40);

    for i in 0..10 {
        mp.update(&start);
        println!("{} {} {}", i, mp.current_val, mp.previous_val);
        thread::sleep(Duration::from_millis(100));
    }
    let context = midi::PortMidi::new().unwrap();
    let device = context.device(2).unwrap();
    let out_device = context.output_port(device, 1024).unwrap();

    test_midi(out_device);
}
