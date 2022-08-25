use std::f32;
use std::time::Instant;
use std::collections::HashMap;


pub enum Updater<'a> {
    Const(&'a str, i8),
    PairedInverseConst(&'a str, i8),
    Sweep(&'a str, i8, i8),
    PairedInverseSweep(&'a str, i8),
    SelectOnZero(&'a str, &'a str, bool)
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


fn random_frequency() -> f32 {
    let r = rand::random::<f64>();
    0.01 + (r / 100.0) as f32
}


fn next_val_from(start: &Instant, freq_hz: f32, min: i8, max: i8) -> i8 {
    let dt = start.elapsed().as_millis() as f32;
    let ang_freq = freq_hz * 2.0 * f32::consts::PI as f32;
    (min as f32 + ((max as f32 - min as f32) * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()))).round() as i8
}


pub fn update<'a, S: SysExComposer, O: Selector, E: Selector>(
    sys_ex: &mut S,
    sweep_state: &mut HashMap::<String, SweepState>,
    osc_selector: &mut O,
    effect_selector: &mut E,
    updaters: &'a [Updater],
    start: &Instant,
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

                let state_val = sweep_state.entry(s).or_insert(SweepState::from(*max, random_frequency()));
                let new_val = next_val_from(&start, state_val.freq_hz, *min, *max);
                *state_val = SweepState::updated_from(&state_val, new_val);
                sys_ex.data(new_val);
            },
            Updater::PairedInverseSweep(key, max) => {
                let s = String::from(*key);
                let sk = [prefix.unwrap(), *key].join("_");

                let normal = '1' == prefix.unwrap().chars().last().unwrap();
                let osc_vol;
                if normal {
                    let master_vol = sweep_state.entry(s).or_insert(SweepState::from(*max, random_frequency()));
                    osc_vol = next_val_from(&start, master_vol.freq_hz, 0, *max);
                    *master_vol = SweepState::updated_from(&master_vol, osc_vol);
                } else {
                    osc_vol = *max - sweep_state.get(&s).unwrap().val;
                }

                let sk_state_val = sweep_state.entry(sk).or_insert(SweepState::from(osc_vol, 0.0));
                *sk_state_val = SweepState::updated_from(&sk_state_val, osc_vol);
                sys_ex.data(osc_vol);
            },
            Updater::SelectOnZero(key, watching, double_byte) => {
                let w = String::from(*watching);
                let idx: u8 = key.chars().last().unwrap().to_digit(10).unwrap() as u8;

                if sweep_state.contains_key(&w) {
                    let ss = sweep_state.get(&w).unwrap();
                    if ss.val == 0 && ss.prev_val != 0 {
                        if 1 == idx {
                            osc_selector.next1();
                            effect_selector.next1();
                        } else {
                            osc_selector.next2();
                            effect_selector.next2();
                        }
                        println!("{} change {}", key, osc_selector.val(idx));
                    }
                }
                if *double_byte {
                    sys_ex.data_double_byte(osc_selector.val(idx) as i16)
                } else {
                    sys_ex.data(osc_selector.val(idx) as i8)
                };
            }
        }
    }
}
