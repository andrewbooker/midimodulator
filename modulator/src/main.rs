extern crate libc;
mod korg;
mod midi;
use crate::korg::{CHANNEL, KorgProgramSysEx};
use crate::midi::{MidiMessage, MidiOut, MidiOutDevices};

use std::{
    f32,
    thread,
    time::{Duration, Instant}
};


struct ConstParam {
    val: i8
}

struct SweepableParam {
    max_val: i16,
    min_val: i16
}

const OSCILLATOR_MODE: ConstParam = ConstParam{ val: 1 };
const NOTE_MODE: ConstParam = ConstParam{ val: 0 };

const DETUNE: SweepableParam = SweepableParam { min_val: -17, max_val: 17 };

struct Sweeper<'a> {
    original: &'a SweepableParam,
    freq_hz: f32,
    previous_val: i16,
    current_val: i16
}

impl Sweeper<'_> {
    fn new<'a>(f_hz: f32, p: &'a SweepableParam) -> Sweeper<'a> {
        Sweeper {
            freq_hz: f_hz,
            original: p,
            current_val: p.max_val,
            previous_val: p.max_val
        }
    }

    fn update(&mut self, since_start: &Instant) {
        let dt = since_start.elapsed().as_millis() as f32;
        let ang_freq = self.freq_hz * 2.0 * f32::consts::PI as f32;

        self.previous_val = self.current_val;
        let val = self.original.min_val as f32 + ((self.original.max_val - self.original.min_val) as f32 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()));
        self.current_val = val.round() as i16;
    }
}

trait ConstSyxExAppender {
    fn append_to(&self, psx: &mut KorgProgramSysEx);
}
trait SyxExAppender {
    fn append_to(&mut self, psx: &mut KorgProgramSysEx);
}

impl ConstSyxExAppender for ConstParam {
    fn append_to(&self, psx: &mut KorgProgramSysEx) {
        psx.data(self.val);
    }
}

fn build_prog_sys_ex(psx: &mut KorgProgramSysEx) {
    psx.name("2021-01-05");
    OSCILLATOR_MODE.append_to(psx);
    NOTE_MODE.append_to(psx);

    let params = json::parse(r#"
    {"list": [{"name": "osc1", "values": [2, 3, 4], "doubleByte": true},
              {"name": "osc1Octave", "minVal": -2, "maxVal": 1},
              {"name": "osc2", "values": [3, 4, 5], "doubleByte": true},
              {"name": "osc2Octave", "minVal": -2, "maxVal": 1},
              {"name": "interval", "constVal": 0}
    ]}"#).unwrap();

    println!("parsing params");
    let arr = &params["list"];
    for i in 0..arr.len() {
        let a = &arr[i];
        println!("{}", a["name"]);
        if a.has_key("constVal") {
            psx.data(a["constVal"].as_i8().unwrap());
        } else if a.has_key("maxVal") {
            psx.data(a["maxVal"].as_i8().unwrap());
        } else if a.has_key("values") {
            let v = &a["values"][0];
            if a.has_key("doubleByte") && a["doubleByte"].as_bool().unwrap() {
                psx.data_double_byte(v.as_i16().unwrap());
            } else {
                psx.data(v.as_i8().unwrap());
            }
        }
    }
    println!("parsing params finished");
}

struct KorgInitSysEx {
    data: [u8; 8]
}

impl KorgInitSysEx {
    fn new() -> KorgInitSysEx {
        KorgInitSysEx {
            data: [0xF0,
                   0x42, // ID of Korg
                   0x30 | CHANNEL, // format ID (3), channel
                   0x36, // 05R/W ID
                   0x4E, // mode change
                   0x03, // program edit
                   0x00,
                   0xF7]
        }
    }
}


fn main() {
    let start = Instant::now();
    let mut mp = Sweeper::new(0.05, &DETUNE);

    for i in 0..10 {
        mp.update(&start);
        println!("{} {} {}", i, mp.current_val, mp.previous_val);
        thread::sleep(Duration::from_millis(100));
    }

    MidiOutDevices::list();

    let mut midi_out = MidiOut::using_device(2);

    let prog28 = MidiMessage::program(28, CHANNEL);
    midi_out.send(&prog28);
    thread::sleep(Duration::from_millis(1000));

    let note = 67;
    let on = MidiMessage::note_on(note, CHANNEL);
    let off = MidiMessage::note_off(note, CHANNEL);

    midi_out.send(&on);
    thread::sleep(Duration::from_millis(2000));
    midi_out.send(&off);
    thread::sleep(Duration::from_millis(1000));

    midi_out.send(&MidiMessage::program(33, CHANNEL));
    thread::sleep(Duration::from_millis(100));

    let kssx = KorgInitSysEx::new();
    midi_out.send_sys_ex(&kssx.data);
    thread::sleep(Duration::from_millis(100));

    let mut kpsx = KorgProgramSysEx::new();
    build_prog_sys_ex(&mut kpsx);

    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }
    let mut port = serialport::new("/dev/ttyUSB0", 38400)
                    .timeout(Duration::from_millis(1000))
                    .open()
                    .expect("Failed to open port");

    port.write(&kpsx.data).expect("Write failed!");
}
