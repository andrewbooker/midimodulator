use std::f32;
use std::time::Instant;
use std::collections::HashMap;


pub enum Updater<'a> {
    Const(&'a str, i8),
    PairedInverseConst(&'a str, i8),
    Sweep(&'a str, i8, i8),
    PairedInverseSweep(&'a str),
    SelectOnZero(&'a str)
}


pub trait SysExComposer {
    fn data(&mut self, d: i8);
    fn data_double_byte(&mut self, d: i16);
    fn name(&mut self, n: &str);
}

pub trait Selector {
    fn next1(&mut self);
    fn next2(&mut self);

    fn val(&self, idx: u8) -> u16;
}

pub struct SweepState {
    pub val: i8, // public so the app can print it
    prev_val: i8,
    freq_hz: f32
}

impl SweepState {
    fn from(val: i8, freq_hz: f32) -> SweepState {
        SweepState {
            val, prev_val: val, freq_hz
        }
    }

    fn updated_from(previous: &SweepState, val: i8) -> SweepState {
        SweepState {
            val, prev_val: previous.val, freq_hz: previous.freq_hz
        }
    }
}

impl Clone for SweepState {
    fn clone(&self) -> Self {
        SweepState { val: self.val, prev_val: self.prev_val, freq_hz: self.freq_hz }
    }
}





pub struct PairedUpdater {
    pub sweep_state: HashMap::<String, SweepState>,
    start: Instant
}

impl PairedUpdater {
    const ALTERNATOR: &'static str = "alternator";
    const ALTERNATOR_MAX: i8 = 99;

    fn random_frequency() -> f32 {
        let r = rand::random::<f64>();
        0.01 + (r / 100.0) as f32
    }

    fn random_between(min: i8, max: i8) -> i8 {
        min + (0.5 + ((max - min) as f32 * rand::random::<f32>())) as i8
    }

    fn next_val_from(start: &Instant, freq_hz: f32, min: i8, max: i8) -> i8 {
        let dt = start.elapsed().as_millis() as f32;
        let ang_freq = freq_hz * 2.0 * f32::consts::PI as f32;
        (min as f32 + ((max as f32 - min as f32) * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()))).round() as i8
    }

    pub fn new() -> PairedUpdater {
        let mut p = PairedUpdater {
            sweep_state: HashMap::<String, SweepState>::new(),
            start: Instant::now()
        };
        p.sweep_state.insert(PairedUpdater::ALTERNATOR.to_string(), SweepState::from(PairedUpdater::random_between(0, PairedUpdater::ALTERNATOR_MAX), PairedUpdater::random_frequency()));
        p
    }

    pub fn sweep_alternator(&mut self) {
        let v = self.sweep_state.get_mut(&PairedUpdater::ALTERNATOR.to_string()).unwrap();
        let nv = PairedUpdater::next_val_from(&self.start, v.freq_hz, 0, PairedUpdater::ALTERNATOR_MAX);
        *v = SweepState::updated_from(&v, nv);
    }

    pub fn update<'a, S: SysExComposer, O: Selector, E: Selector>(
        &mut self,
        sys_ex: &mut S,
        osc_selector: &mut O,
        effect_selector: &mut E,
        updaters: &'a [Updater],
        prefix: Option<&str>)
    {
        for u in updaters {
            match u {
                Updater::Const(_, c) => {
                    sys_ex.data(*c);
                },
                Updater::PairedInverseConst(_, c) => {
                    let inverse = '2' == prefix.unwrap().chars().last().unwrap();
                    sys_ex.data(if inverse { *c } else { 0 });
                },
                Updater::Sweep(key, min, max) => {
                    let s = if prefix.is_none() { String::from(*key) } else { [prefix.unwrap(), *key].join("_") };

                    let state_val = self.sweep_state.entry(s).or_insert(SweepState::from(*max, PairedUpdater::random_frequency()));
                    let new_val = PairedUpdater::next_val_from(&self.start, state_val.freq_hz, *min, *max);
                    *state_val = SweepState::updated_from(&state_val, new_val);
                    sys_ex.data(new_val);
                },
                Updater::PairedInverseSweep(_) => {
                    let idx = prefix.unwrap().chars().last().unwrap().to_digit(10).unwrap() as u8;
                    let inverse = (idx % 2) == 0;
                    let v = self.sweep_state.get(&PairedUpdater::ALTERNATOR.to_string()).unwrap();
                    if inverse {
                        sys_ex.data(PairedUpdater::ALTERNATOR_MAX - v.val);
                    } else {
                        sys_ex.data(v.val);
                    }
                },
                Updater::SelectOnZero(key) => {
                    let idx = key.chars().last().unwrap().to_digit(10).unwrap() as u8;
                    let inverse = (idx % 2) == 0;

                    let v = self.sweep_state.get(&PairedUpdater::ALTERNATOR.to_string()).unwrap();
                    let test_v = if inverse { PairedUpdater::ALTERNATOR_MAX } else { 0 };

                    if v.val == test_v && v.prev_val != test_v {
                        if 1 == idx {
                            osc_selector.next1();
                            effect_selector.next1();
                        } else {
                            osc_selector.next2();
                            effect_selector.next2();
                        }
                        println!("{} change {}", key, osc_selector.val(idx));
                    }
                    sys_ex.data_double_byte(osc_selector.val(idx) as i16);
                }
            }
        }
    }
}
