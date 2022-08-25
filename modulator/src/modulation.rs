

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
