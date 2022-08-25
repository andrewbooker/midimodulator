

use crate::modulation::SysExComposer;

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
