
use crate::modulation::{
    SysExComposer,
    Selector,
    Updater
};
use rand::prelude::SliceRandom;

pub const CHANNEL: u8 = 0;

pub struct KorgProgramSysEx {
    pos: usize,
    pub data: [u8; 196 + 6]
}


impl KorgProgramSysEx {
    pub fn new() -> KorgProgramSysEx {
        let mut s = KorgProgramSysEx {
            pos: 1,
            data: [0; 196 + 6]
        };
        s.data[0] = 0xF0;
        s.data[1] = 0x42;
        s.data[2] = 0x30 | CHANNEL;
        s.data[3] = 0x36;
        s.data[4] = 0x40;
        s.data[196 + 5] = 0xF7;
        s
    }
}

impl SysExComposer for KorgProgramSysEx {
    fn data(&mut self, d: i8) {
        self.data[self.pos + 5] = (0x7F & d) as u8;
        let shift: usize = 7 - (self.pos - 1) % 8;
        let block_idx: usize = 8 * (self.pos / 8);
        let carry: u8 = (d as u8 & 0x80) >> shift;
        self.data[block_idx + 5] |= carry;
        self.pos += if shift == 1 { 2 } else { 1 };
    }

    fn data_double_byte(&mut self, d: i16) {
        self.data(d as i8);
        self.data((d >> 8) as i8);
    }

    fn name(&mut self, n: &str) {
        for c in n.chars() {
            self.data(c as i8);
        }
    }
}


pub struct KorgInitSysEx {
    pub data: [u8; 8]
}

impl KorgInitSysEx {
    pub fn new(mode: u8) -> KorgInitSysEx {
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

pub struct KorgSingleParamSysEx {
    pub data: [u8; 10]
}


impl KorgSingleParamSysEx {
    pub fn new(p: u8, v: u8) -> KorgSingleParamSysEx {
        KorgSingleParamSysEx {
            data: [0xF0,
                   0x42, // ID of Korg
                   0x30 | CHANNEL, // format ID (3), channel
                   0x36, // 05R/W ID
                   0x41, // parameter change
                   p & 0x7F, // lsb parameter #
                   (p >> 7) & 0x7F, // msb
                   v & 0x7F, // lsb value
                   (v >> 7) & 0x7F, // msb
                   0xF7]
        }
    }
}

pub const ENV_TIME_LOW: i8 = 1;
pub const ENV_TIME_HIGH: i8 = 10;

pub const PROGRAM_SPEC: [Updater; 28] = [
    Updater::Const("oscillatorMode", 1),
    Updater::Const("noteMode", 0),
    Updater::SelectOnZero("osc1"),
    Updater::Const("osc1Register", 0),
    Updater::SelectOnZero("osc2"),
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

pub const OSC_SPEC: [Updater; 47] = [
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
    Updater::Sweep("vdfCutoff", 40, 80),
    Updater::Const("vdfCutoffKeybTrackKey", 64),
    Updater::Const("vdfCutoffKeybTrackIntensity", 64),
    Updater::Sweep("vdfEgIntensity", 20, 99),
    Updater::Const("vdfEgTimeKeybTrack", 50),
    Updater::Const("vdfEgTimeVelocitySens", 20),
    Updater::Const("vdfEgIntensityVelocitySens", 70),
    Updater::Sweep("env_filter_attackTime", 1, 10),
    Updater::Sweep("env_filter_attackLevel", -10, 90),
    Updater::Sweep("env_filter_decayTime", ENV_TIME_LOW, ENV_TIME_HIGH),
    Updater::Sweep("env_filter_breakPoint", -10, 90),
    Updater::Sweep("env_filter_slopeTime", ENV_TIME_LOW, ENV_TIME_HIGH),
    Updater::Sweep("env_filter_sustainLevel", -30, 90),
    Updater::Sweep("env_filter_releaseTime", 30, 60),
    Updater::Sweep("env_filter_releaseLevel", -90, 90),
    Updater::PairedInverseSweep("vol"),
    Updater::Const("oscKeybTrackKey", 0),
    Updater::Const("amplKeybTrackKeyIntensity", 0),
    Updater::Const("amplVelocitySens", 11),
    Updater::Const("amplEgTimeKeybTrack", 50),
    Updater::Const("amplEgTimeVelocitySens", 10),
    Updater::Sweep("env_amplitude_attackTime", 1, 10),
    Updater::Sweep("env_amplitude_attackLevel", 60, 80),
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
    Updater::Sweep("filterQ", 20, 99),
    Updater::Const("colourVelocitySens", 56),
    Updater::Const("vdfVdaKeyboardTrackMode", 0),
    Updater::Const("panCentre", 0x0F) // pan 0: A15, 0x0F: centre, 0x1E: B15
];



pub struct KorgOscSelector {
    osc1: u16,
    osc2: u16
}

impl KorgOscSelector {
    const OSCILLATOR_RANGES: [(u16, u16); 26] = [
        (0, 10),
        (11, 42),
        (43, 59),
        (61, 0),
        (63, 0),
        (70, 98),
        (99, 105),
        (106, 115),
        (129, 0),
        (132, 143),
        (145, 153),
        (155, 0),
        (159, 0),
        (161, 0),
        (167, 170),
        (171, 0),
        (172, 173),
        (175, 184),
        (206, 207),
        (209, 210),
        (221, 226),
        (252, 256),
        (260, 0),
        (268, 0),
        (316, 333),
        (335, 337)
    ];

    fn expand(a: &[(u16, u16)]) -> Vec<u16> {
        let mut ret = Vec::new();
        for r in a {
            if r.1 == 0 {
                ret.push(r.0);
            } else {
                for osc in r.0..(r.1 + 1) {
                    ret.push(osc);
                }
            }
        }
        ret
    }

    fn random_osc() -> u16 {
        *KorgOscSelector::expand(&KorgOscSelector::OSCILLATOR_RANGES).choose(&mut rand::thread_rng()).unwrap()
    }

    pub fn new() -> KorgOscSelector {
        KorgOscSelector {
            osc1: KorgOscSelector::random_osc(),
            osc2: KorgOscSelector::random_osc()
        }
    }
}

impl Selector for KorgOscSelector {
    fn next1(&mut self) {
        self.osc1 = KorgOscSelector::random_osc();
    }

    fn next2(&mut self) {
        self.osc2 = KorgOscSelector::random_osc();
    }

    fn val(&self, at: u8) -> u16 {
        if at == 1 {
            self.osc1
        } else {
            self.osc2
        }
    }
}



pub type FxUpdater<'a> = [Updater<'a>; 10];

pub struct Effect<'a> {
    number: i8,
    mix: i8,
    pub updater: FxUpdater<'a>
}

const PHASER: Effect = Effect {
    number: 32,
    mix: 50,
    updater: [
        Updater::Sweep("phaserDepth", 50, 99),
        Updater::Sweep("phaserSpeed", 20, 99), // could make this const as the modulation varies it
        Updater::Const("phaserWaveform", 0), // 0: sine, 1: tri
        Updater::Sweep("phaserFeedback", -99, 99),
        Updater::Sweep("phaserManual", 5, 65),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("eff_modSource", 4), // 4, or 5 for the other effect
        Updater::Const("eff_modAmount", 15), // 15
    ]
};

const TREMOLO: Effect = Effect {
    number: 36,
    mix: 99,
    updater: [
        Updater::Sweep("tremoloDepth", 50, 99),
        Updater::Sweep("tremoloSpeed", 64, 127), // should be 200 but only supporting i8 atm
        Updater::Const("tremoloWaveform", 0), // 0: sine, 1: tri
        Updater::Sweep("tremoloWaveShape", -99, 99),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("eff_modSource", 0), // don't bother with modulation as it only affects the balance
        Updater::Const("eff_modAmount", 0)
    ]
};


const DISTORTION: Effect = Effect {
    number: 30,
    mix: 50,
    updater: [
        Updater::Sweep("distDrive", 1, 88),
        Updater::Sweep("distHotSpot", 2, 60),
        Updater::Sweep("distResonance", 5, 77),
        Updater::Const("distOut", 50),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("", 0),
        Updater::Const("eff_modSource", 4), // 4, or 5 for the other effect
        Updater::Const("eff_modAmount", 15) // 15
    ]
};


const AVAILABLE_EFFECTS: [Effect; 3] = [
    PHASER,
    TREMOLO,
    DISTORTION
];


pub struct KorgEffectSelector<'a> {
    pub eff1: &'a Effect<'a>,
    pub eff2: &'a Effect<'a>
}

impl <'a>KorgEffectSelector<'a> {
    pub fn new() -> KorgEffectSelector<'a> {
        KorgEffectSelector {
            eff1: &AVAILABLE_EFFECTS.choose(&mut rand::thread_rng()).unwrap(),
            eff2: &AVAILABLE_EFFECTS.choose(&mut rand::thread_rng()).unwrap()
        }
    }

    pub fn pre_eff(&self) -> FxUpdater<'a> {
        [
            Updater::Const("", 0),
            Updater::Const("eff1_number", self.eff1.number),
            Updater::Const("eff2_number", self.eff2.number),
            Updater::Const("eff1_level_A", self.eff1.mix),
            Updater::Const("eff1_level_B", self.eff1.mix),
            Updater::Const("eff2_level_C", self.eff2.mix),
            Updater::Const("eff2_level_D", self.eff2.mix),
            Updater::Const("pan3", 101),
            Updater::Const("pan4", 1),
            Updater::Const("eff_routing", 0x10 | 0x0F) // routing | enable
        ]
    }
}


impl <'a>Selector for KorgEffectSelector<'a> {
    fn next1(&mut self) {
        self.eff1 = &AVAILABLE_EFFECTS.choose(&mut rand::thread_rng()).unwrap();
    }

    fn next2(&mut self) {
        self.eff2 = &AVAILABLE_EFFECTS.choose(&mut rand::thread_rng()).unwrap();
    }

    fn val(&self, _: u8) -> u16 {
        0
    }
}
