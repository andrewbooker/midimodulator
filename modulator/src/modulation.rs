

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
