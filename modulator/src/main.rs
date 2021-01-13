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
    Sweep(&'a str, i8, i8),
    PairedSweep(&'a str),
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

const OSC_SPEC: [Updater; 44] = [
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
    Updater::PairedSweep("vol"),
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
    Updater::Const("cdSend", 0), // ConstProgSetting< 0x99 > m_cdSend2( "cdSend2", g_osc2Settings ); ... Paired??
    Updater::Sweep("filterQ", 40, 99)
];


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

struct SweepState {
    val: i8,
    freq_hz: f32
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
            Updater::Sweep(key, min, max) => {
                let s = if prefix.is_none() { String::from(*key) } else { [prefix.unwrap(), *key].join("_") };

                let state_val = sweep_state.entry(s).or_insert(SweepState { val: *max, freq_hz: 0.05 });
                let dt = start.elapsed().as_millis() as f32;
                let ang_freq = state_val.freq_hz * 2.0 * f32::consts::PI as f32;
                let new_val = (*min as f32 + ((*max as f32 - *min as f32) * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()))).round() as i8;
                *state_val = SweepState { val: new_val, freq_hz: 0.05 };
                kpsx.data(new_val);
            },
            Updater::PairedSweep(key) => {
                let s = String::from(*key);
                let state_val = sweep_state.entry(s).or_insert(SweepState { val: 99, freq_hz: 0.05 });

                let inverse = '2' == prefix.unwrap().chars().last().unwrap();
                let sk = [prefix.unwrap(), *key].join("_");
                if inverse {
                    let new_val = 99 - state_val.val;
                    sweep_state.entry(sk).or_insert(SweepState { val: new_val, freq_hz: 0.0 });
                    kpsx.data(new_val);
                } else {
                    // as sweep
                    let dt = start.elapsed().as_millis() as f32;
                    let ang_freq = state_val.freq_hz * 2.0 * f32::consts::PI as f32;
                    let new_val = (99.0 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos())).round() as i8;
                    *state_val = SweepState { val: new_val, freq_hz: 0.05 };
                    sweep_state.entry(sk).or_insert(SweepState { val: new_val, freq_hz: 0.0 });
                    kpsx.data(new_val);
                }
            },
            Updater::SelectOnZero(key, watching, double_byte) => {
                let s = if prefix.is_none() { String::from(*key) } else { [prefix.unwrap(), *key].join("_") };
                let w = String::from(*watching);

                let state_val = selector_state.entry(s).or_insert(9);
                if sweep_state.contains_key(&w) && sweep_state.get(&w).unwrap().val == 0 {
                    *state_val = 99;
                }
                if *double_byte { kpsx.data_double_byte(*state_val) } else { kpsx.data(*state_val as i8) };
            }
        }
    }
}


fn main() {
    let mut sweep_state = HashMap::<String, SweepState>::new();
    let mut selector_state = HashMap::<String, i16>::new();

    let start = Instant::now();

    MidiOutDevices::list();

    let mut midi_out = MidiOut::using_device(2);
    let kssx = KorgInitSysEx::new(0x02); // select prog
    midi_out.send_sys_ex(&kssx.data);
    thread::sleep(Duration::from_millis(100));

    midi_out.send(&MidiMessage::program(33, CHANNEL));
    thread::sleep(Duration::from_millis(100));

    let kssx = KorgInitSysEx::new(0x03); // edit prog
    midi_out.send_sys_ex(&kssx.data);
    thread::sleep(Duration::from_millis(100));

    let mut kpsx = KorgProgramSysEx::new();
    kpsx.name("2021-01-05");

    update(&mut kpsx, &mut sweep_state, &mut selector_state, &PROGRAM_SPEC, &start, None);
    update(&mut kpsx, &mut sweep_state, &mut selector_state, &OSC_SPEC, &start, Some("osc1"));
    update(&mut kpsx, &mut sweep_state, &mut selector_state, &OSC_SPEC, &start, Some("osc2"));

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
    note_test(&mut midi_out, 33);
}
