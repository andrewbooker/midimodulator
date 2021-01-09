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

    pub fn data(&mut self, d: u8) -> &mut KorgProgramSysEx {
        self.data[self.pos + 5] = 0x7F & d;
        let shift: usize = 7 - (self.pos - 1) % 8;
        let block_idx: usize = 8 * (self.pos / 8);
        let carry: u8 = (d & 0x80) >> shift;
        self.data[block_idx + 5] |= carry;
        self.pos += if shift == 1 { 2 } else { 1 };
        self
    }

    pub fn data_double_byte(&mut self, d: u16) -> &mut KorgProgramSysEx {
        self.data(d as u8);
        self.data((d >> 8) as u8);
        self
    }

    pub fn name(&mut self, n: &str) -> &mut KorgProgramSysEx {
        for c in n.chars() {
            self.data(c as u8);
        }
        self
    }
}
