extern crate libc;
mod korg;
mod midi;
use crate::korg::{CHANNEL, KorgProgramSysEx};
use crate::midi::{MidiMessage, MidiOut, MidiOutDevices};

use std::{
    f32,
    thread,
    time::{Duration, Instant},
    collections::HashMap
};


const OSCILLATORS: [i16; 11] = [0,1,2,3,4,5,6,7,8,9,10];


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


enum Updater<'a> {
    Const(&'a str, i8),
    Sweep(&'a str, i8, i8),
    SelectOnZero(&'a str, &'a str, bool)
}

const PROGRAM_SPEC: [Updater; 28] = [
    Updater::Const("oscillatorMode", 1),
    Updater::Const("noteMode", 0),
    Updater::SelectOnZero("osc1", "vol1", true),
    Updater::Const("osc1Register", 0),
    Updater::SelectOnZero("osc2", "vol2", true),
    Updater::Const("osc2Register", 0),
    Updater::Const("octave", 0),
    Updater::Sweep("detune", -17, 17),
    Updater::Const("delay", 0),

    Updater::Const("env_pitch_startLevel", 0),
    Updater::Sweep("env_pitch_attackTime", 1, 4),
    Updater::Sweep("env_pitch_attackLevel", -7, 7),
    Updater::Sweep("env_pitch_decayTime", 10, 30),
    Updater::Sweep("env_pitch_releaseTime", 10, 30),
    Updater::Sweep("env_pitch_releaseLevel", -8, 0),

    Updater::Const("pitchEgTimeVelocitySens", 0),
    Updater::Const("pitchEgLevelVelocitySens", 0),
    Updater::Const("cutoffTypeDetails", 0),  // cutoff type (bits 1-4 = waveform 0=TRI, bit5=osc1 enable, bit6=osc2 enable, bit7=key sync)
    Updater::Sweep("modFreq", 20, 99),
    Updater::Sweep("modDelay", 1, 40),
    Updater::Sweep("modIntensity", 1, 40),

    Updater::Const("pitchBendRange", 0),
    Updater::Const("vdfCutoff", 0),
    Updater::Const("vdfModulationIntensity", 0),
    Updater::Const("vdaAmplitude", 0),
    Updater::Const("joystickPitchBendRange", 0),
    Updater::Const("joystickVdfSweepIntensity", 0),
    Updater::Const("joystickVdfModulationIntensity", 0)
];

const OSC_SPEC: [Updater; 2] = [
    Updater::Sweep("pitchEgIntensity", 1, 20),
    Updater::Const("pitchWaveform", 0) // bits 1-4 = waveform, bit7=key sync)
];

struct SweepState {
    val: i8,
    freq_hz: f32
}


fn note_test(midi_out: &mut MidiOut, prg: u8) {
    let prog28 = MidiMessage::program(prg, CHANNEL);
    midi_out.send(&prog28);
    thread::sleep(Duration::from_millis(1000));

    let note = 67;
    let on = MidiMessage::note_on(note, CHANNEL);
    let off = MidiMessage::note_off(note, CHANNEL);

    midi_out.send(&on);
    thread::sleep(Duration::from_millis(2000));
    midi_out.send(&off);
    thread::sleep(Duration::from_millis(1000));
}


fn update<'a>(kpsx: &mut KorgProgramSysEx,
              sweep_state: &mut HashMap::<&'a str, SweepState>,
              selector_state: &mut HashMap::<&'a str, i16>,
              updaters: &'a [Updater],
              start: &Instant)
{
    for u in updaters {
        match u {
            Updater::Const(_, c) => {
                kpsx.data(*c);
            },
            Updater::Sweep(s, min, max) => {
                let state_val = sweep_state.entry(s).or_insert(SweepState { val: *max, freq_hz: 0.05 });
                let dt = start.elapsed().as_millis() as f32;
                let ang_freq = state_val.freq_hz * 2.0 * f32::consts::PI as f32;
                let new_val = (*min as f32 + ((*max - *min) as f32 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()))).round() as i8;
                *state_val = SweepState { val: new_val, freq_hz: 0.05 };
                kpsx.data(new_val);
            },
            Updater::SelectOnZero(s, watching, double_byte) => {
                let state_val = selector_state.entry(s).or_insert(9);
                if sweep_state.contains_key(watching) && sweep_state.get(watching).unwrap().val == 0 {
                    *state_val = 99;
                }
                if *double_byte { kpsx.data_double_byte(*state_val) } else { kpsx.data(*state_val as i8) };
            }
        }
    }
}


fn main() {
    let mut sweep_state = HashMap::<&str, SweepState>::new();
    let mut selector_state = HashMap::<&str, i16>::new();

    let start = Instant::now();

    MidiOutDevices::list();

    let mut midi_out = MidiOut::using_device(2);
    note_test(&mut midi_out, 28);

    midi_out.send(&MidiMessage::program(33, CHANNEL));
    thread::sleep(Duration::from_millis(100));

    let kssx = KorgInitSysEx::new();
    midi_out.send_sys_ex(&kssx.data);
    thread::sleep(Duration::from_millis(100));

    let mut kpsx = KorgProgramSysEx::new();
    kpsx.name("2021-01-05");

    update(&mut kpsx, &mut sweep_state, &mut selector_state, &PROGRAM_SPEC, &start);

    for (key, val) in &sweep_state {
        println!("{}: {}", key, val.val);
    }
    for (key, val) in &selector_state {
        println!("{}: {}", key, val);
    }

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
