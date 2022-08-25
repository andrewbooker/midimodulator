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
    pub val: i8, // can eventually be private
    pub prev_val: i8, // can eventually be private
    pub freq_hz: f32 // can eventually be private
}

impl SweepState {
    pub fn from(val: i8, freq_hz: f32) -> SweepState { // can eventually be private
        SweepState {
            val, prev_val: val, freq_hz
        }
    }

    pub fn updated_from(previous: &SweepState, val: i8) -> SweepState { // can eventually be private
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
                let freq_hz = random_frequency();
                let s = if prefix.is_none() { String::from(*key) } else { [prefix.unwrap(), *key].join("_") };

                let state_val = sweep_state.entry(s).or_insert(SweepState::from(*max, freq_hz));
                let dt = start.elapsed().as_millis() as f32;
                let ang_freq = state_val.freq_hz * 2.0 * f32::consts::PI as f32;
                let new_val = (*min as f32 + ((*max as f32 - *min as f32) * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()))).round() as i8;
                *state_val = SweepState::updated_from(&state_val, new_val);
                sys_ex.data(new_val);
            },
            Updater::PairedInverseSweep(key, max) => {
                let vol_freq_hz = random_frequency();
                let s = String::from(*key);
                let sk = [prefix.unwrap(), *key].join("_");

                let normal = '1' == prefix.unwrap().chars().last().unwrap();
                let osc_vol;
                if normal {
                    let master_vol = sweep_state.entry(s).or_insert(SweepState::from(*max, vol_freq_hz));

                    // as sweep
                    let dt = start.elapsed().as_millis() as f32;
                    let ang_freq = master_vol.freq_hz * 2.0 * f32::consts::PI as f32;
                    osc_vol = (*max as f32 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos())).round() as i8;
                    *master_vol = SweepState::updated_from(&master_vol, osc_vol);
                } else {
                    osc_vol = 99 - sweep_state.get(&s).unwrap().val;
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
