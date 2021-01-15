extern crate libc;

mod korg;
mod midi;

use crate::korg::{CHANNEL, KorgProgramSysEx};
use crate::midi::{MidiMessage, MidiOut, MidiOutDevices};
use std::{
    f32,
    thread,
    time::{Duration, Instant},
    collections::HashMap,
    sync::mpsc
};
use rand::prelude::SliceRandom;


struct KorgInitSysEx {
    data: [u8; 8]
}

impl KorgInitSysEx {
    fn new(mode: u8) -> KorgInitSysEx {
        KorgInitSysEx {
            data: [0xF0,
                   0x42, // ID of Korg
                   0x30 | CHANNEL, // format ID (3), channel
                   0x36, // 05R/W ID
                   0x4E, // mode change
                   mode,
                   0x00,
                   0xF7]
        }
    }
}


enum Updater<'a> {
    Const(&'a str, i8),
    PairedInverseConst(&'a str, i8),
    Sweep(&'a str, i8, i8),
    PairedInverseSweep(&'a str, i8),
    SelectOnZero(&'a str, &'a str, bool)
}

const ENV_TIME_LOW: i8 = 1;
const ENV_TIME_HIGH: i8 = 10;

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
    Updater::Sweep("env_pitch_decayTime", ENV_TIME_LOW, ENV_TIME_HIGH),
    Updater::Sweep("env_pitch_releaseTime", ENV_TIME_LOW, ENV_TIME_HIGH),
    Updater::Sweep("env_pitch_releaseLevel", -8, 0),

    Updater::Const("pitchEgTimeVelocitySens", 0),
    Updater::Const("pitchEgLevelVelocitySens", 0),
    Updater::Const("cutoffTypeDetails", 0), // cutoff type (bits 1-4 = waveform 0=TRI, bit5=osc1 enable, bit6=osc2 enable, bit7=key sync)
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

const OSC_SPEC: [Updater; 47] = [
    Updater::Sweep("pitchEgIntensity", 1, 20),
    Updater::Const("pitchWaveform", 0), // bits 1-4 = waveform, bit7=key sync)
    Updater::Sweep("pitchEgFreq", 10, 50),
    Updater::Sweep("pitchEgDelay", 5, 50),
    Updater::Sweep("pitchEgFadeIn", 3, 20),
    Updater::Sweep("pitchModulationIntensity", 1, 10),
    Updater::Const("pitchFreqModKeyTracking", -5),
    Updater::Const("pitchModIntensityAftertouch", 0),
    Updater::Const("pitchModIntensityJoystick", 0),
    Updater::Const("pitchFreqModAftertouchJoystick", 0),
    Updater::Sweep("vdfCutoff", 20, 80),
    Updater::Const("vdfCutoffKeybTrackKey", 64),
    Updater::Const("vdfCutoffKeybTrackIntensity", 64),
    Updater::Sweep("vdfEgIntensity", 20, 99),
    Updater::Const("vdfEgTimeKeybTrack", 50),
    Updater::Const("vdfEgTimeVelocitySens", 20),
    Updater::Const("vdfEgIntensityVelocitySens", 70),
    Updater::Sweep("env_filter_attackTime", 1, 10),
    Updater::Sweep("env_filter_attackLevel", -90, 90),
    Updater::Sweep("env_filter_decayTime", ENV_TIME_LOW, ENV_TIME_HIGH),
    Updater::Sweep("env_filter_breakPoint", -90, 90),
    Updater::Sweep("env_filter_slopeTime", ENV_TIME_LOW, ENV_TIME_HIGH),
    Updater::Sweep("env_filter_sustainLevel", -90, 90),
    Updater::Sweep("env_filter_releaseTime", 30, 60),
    Updater::Sweep("env_filter_releaseLevel", -90, 90),
    Updater::PairedInverseSweep("vol", 99),
    Updater::Const("oscKeybTrackKey", 0),
    Updater::Const("amplKeybTrackKeyIntensity", 0),
    Updater::Const("amplVelocitySens", 11),
    Updater::Const("amplEgTimeKeybTrack", 50),
    Updater::Const("amplEgTimeVelocitySens", 10),
    Updater::Sweep("env_amplitude_attackTime", 1, 10),
    Updater::Sweep("env_amplitude_attackLevel", 40, 90),
    Updater::Sweep("env_amplitude_decayTime", ENV_TIME_LOW, ENV_TIME_HIGH),
    Updater::Sweep("env_amplitude_breakPoint", 40, 90),
    Updater::Sweep("env_amplitude_slopeTime", 5, 60),
    Updater::Sweep("env_amplitude_sustainLevel", 40, 90),
    Updater::Sweep("env_amplitude_releaseTime", 5, 80),
    Updater::Const("freq_EgTimeKeybTrackSwitchPolarity", 0),
    Updater::Const("freq_EgTimeVelocitySwitchPolarity", 0),
    Updater::Const("ampl_EgTimeKeybTrackSwitchPolarity", 0),
    Updater::Const("ampl_EgTimeVelocitySwitchPolarity", 0),
    Updater::PairedInverseConst("cdSend", -103), // 0x99
    Updater::Sweep("filterQ", 40, 99),
    Updater::Const("colourVelocitySens", 56),
    Updater::Const("vdfVdaKeyboardTrackMode", 0),
    Updater::Const("panCentre", 0x0F) // pan 0: A15, 0x0F: centre, 0x1E: B15
];

const PRE_FX: [Updater; 10] = [
    Updater::Const("", 0),
    Updater::Const("eff1_number", 0),
    Updater::Const("eff2_number", 0),
    Updater::Const("eff1_level_A", 0),
    Updater::Const("eff1_level_B", 0),
    Updater::Const("eff2_level_C", 0),
    Updater::Const("eff2_level_D", 0),
    Updater::Const("pan3", 101),
    Updater::Const("pan4", 1),
    Updater::Const("eff_routing", 0x10 | 0x0F) // routing | enable
];


fn note_test(midi_out: &mut MidiOut, note: u8) {
    let on = MidiMessage::note_on(note, CHANNEL);
    let off = MidiMessage::note_off(note, CHANNEL);

    midi_out.send(&on);
    thread::sleep(Duration::from_millis(325));
    midi_out.send(&off);
    thread::sleep(Duration::from_millis(125));
}

struct SweepState {
    val: i8,
    freq_hz: f32
}

impl Clone for SweepState {
    fn clone(&self) -> Self {
        SweepState { val: self.val, freq_hz: self.freq_hz }
    }
}

fn random_frequency() -> f32 {
    let r = rand::random::<f64>();
    0.01 + (r / 100.0) as f32
}

const OSCILLATORS: [i16; 11] = [0,1,2,3,4,5,6,7,8,9,10];
fn random_osc() -> i16 {
    *OSCILLATORS.choose(&mut rand::thread_rng()).unwrap()
}

fn update<'a>(kpsx: &mut KorgProgramSysEx,
              sweep_state: &mut HashMap::<String, SweepState>,
              selector_state: &mut HashMap::<String, i16>,
              updaters: &'a [Updater],
              start: &Instant,
              prefix: Option<&str>)
{
    for u in updaters {
        match u {
            Updater::Const(_, c) => {
                kpsx.data(*c);
            },
            Updater::PairedInverseConst(_, c) => {
                let inverse = '2' == prefix.unwrap().chars().last().unwrap();
                kpsx.data(if inverse { *c } else { 0 });
            },
            Updater::Sweep(key, min, max) => {
                let freq_hz = random_frequency();
                let s = if prefix.is_none() { String::from(*key) } else { [prefix.unwrap(), *key].join("_") };

                let state_val = sweep_state.entry(s).or_insert(SweepState { val: *max, freq_hz: freq_hz });
                let dt = start.elapsed().as_millis() as f32;
                let ang_freq = state_val.freq_hz * 2.0 * f32::consts::PI as f32;
                let new_val = (*min as f32 + ((*max as f32 - *min as f32) * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()))).round() as i8;
                *state_val = SweepState { val: new_val, freq_hz: state_val.freq_hz };
                kpsx.data(new_val);
            },
            Updater::PairedInverseSweep(key, max) => {
                let vol_freq_hz = random_frequency();
                let s = String::from(*key);
                let sk = [prefix.unwrap(), *key].join("_");

                let normal = '1' == prefix.unwrap().chars().last().unwrap();
                let osc_vol;
                if normal {
                    let master_vol = sweep_state.entry(s).or_insert(SweepState { val: *max, freq_hz: vol_freq_hz });

                    // as sweep
                    let dt = start.elapsed().as_millis() as f32;
                    let ang_freq = master_vol.freq_hz * 2.0 * f32::consts::PI as f32;
                    osc_vol = (*max as f32 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos())).round() as i8;
                    *master_vol = SweepState { val: osc_vol, freq_hz: master_vol.freq_hz };
                } else {
                    osc_vol = 99 - sweep_state.get(&s).unwrap().val;
                }

                let sk_state_val = sweep_state.entry(sk).or_insert(SweepState { val: osc_vol, freq_hz: 0.0 });
                *sk_state_val = SweepState { val: osc_vol, freq_hz: 0.0 };
                kpsx.data(osc_vol);
            },
            Updater::SelectOnZero(key, watching, double_byte) => {
                let s = if prefix.is_none() { String::from(*key) } else { [prefix.unwrap(), *key].join("_") };
                let w = String::from(*watching);

                let state_val = selector_state.entry(s).or_insert(random_osc());
                if sweep_state.contains_key(&w) && sweep_state.get(&w).unwrap().val == 0 {
                    *state_val = random_osc();
                }
                if *double_byte { kpsx.data_double_byte(*state_val) } else { kpsx.data(*state_val as i8) };
            }
        }
    }
}


fn main() {
    MidiOutDevices::list();

    let mut midi_out = MidiOut::using_device(2);
    {
        let kssx = KorgInitSysEx::new(0x02); // select prog
        midi_out.send_sys_ex(&kssx.data);
    }

    midi_out.send(&MidiMessage::program(33, CHANNEL));
    thread::sleep(Duration::from_millis(100));

    {
        let kssx = KorgInitSysEx::new(0x03); // edit prog
        midi_out.send_sys_ex(&kssx.data);
    }

    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }

    let (cmd_dump_tx, cmd_dump_rx) = mpsc::channel();
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();
    let (res_tx, res_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut port = serialport::new("/dev/ttyUSB0", 38400)
                    .timeout(Duration::from_millis(1000))
                    .open()
                    .expect("Failed to open port");

        let mut sweep_state = HashMap::<String, SweepState>::new();
        let mut selector_state = HashMap::<String, i16>::new();

        let start = Instant::now();

        loop {
            let mut kpsx = KorgProgramSysEx::new();
            kpsx.name("2021-01-13");

            update(&mut kpsx, &mut sweep_state, &mut selector_state, &PROGRAM_SPEC, &start, None);
            update(&mut kpsx, &mut sweep_state, &mut selector_state, &OSC_SPEC, &start, Some("osc1"));
            update(&mut kpsx, &mut sweep_state, &mut selector_state, &OSC_SPEC, &start, Some("osc2"));
            update(&mut kpsx, &mut sweep_state, &mut selector_state, &PRE_FX, &start, None);

            port.write(&kpsx.data).expect("Write failed!");
            thread::sleep(Duration::from_millis(100));

            match cmd_dump_rx.try_recv() {
                Ok(_) => {
                    res_tx.send(sweep_state.clone()).unwrap();
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
                'l' => {
                    cmd_dump_tx.send(()).unwrap();
                    for res in &res_rx {
                        for (key, val) in &res {
                            println!("{}: {}", key, val.val);
                        }
                    }
                    continue;
                },
                'q' => {
                    cmd_stop_tx.send(()).unwrap();
                    break;
                },
                _ => {}
            }
        }
    });

    let mut n = 0;
    loop {
        note_test(&mut midi_out, 40 + (2 * n));
        n += 1;
        if n > 20 {
            n = 0;
        }
        match cmd_stop_rx.try_recv() {
            Ok(_) => {
                println!("stopping...");
                break;
            },
            _ => {}
        }
    }
    thread::sleep(Duration::from_millis(2000));
}
