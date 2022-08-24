
use crate::utils::today;


pub const CHANNEL_D110: u8 = 9;


pub struct D110SysEx {
    sum: u32,
    data: Vec<u8>
}


impl D110SysEx {
    fn new() -> D110SysEx {
        D110SysEx {
            sum: 0,
            data: Vec::<u8>::new()
        }
    }

    fn data_u8(&mut self, v: u8) {
        self.data.push(v);
        self.sum += v as u32;
    }

    fn data(&mut self, values: Vec<u8>) {
        for v in values {
            self.data_u8(v);
        }
    }

    fn data_str(&mut self, st: &str) {
        for s in st.chars() {
            self.data_u8(s as u8);
        }
    }

    fn checksum(&self) -> u32 {
        128 - (self.sum % 128)
    }

    pub fn to_send(&self) -> Vec<u8> {
        const SYS_EX_START: u8 = 0xF0;
        const SYS_EX_END: u8 = 0xF7;

        const HEADER: [u8; 5] = [
            SYS_EX_START,
            0x41, // ID of Roland
            0x10, // device
            0x16, // model
            0x12 // command
        ];
        let mut msg = Vec::<u8>::new();
        for c in HEADER.iter() {
            msg.push(*c);
        }
        for d in self.data.iter() {
            msg.push(*d);
        }

        msg.push(self.checksum() as u8);
        msg.push(SYS_EX_END);

        msg
    }
}

pub fn init_d110() -> D110SysEx {
    const RES_ALLOWANCE_FOR_PARTIALS: [u8; 9] = [32, 0, 0, 0, 0, 0, 0, 0, 0];
    const MIDI_CHANNELS: [u8; 9] = [16, 9, 9, 9, 9, 9, 9, 9, 1];

    let mut sys_ex = D110SysEx::new();

    sys_ex.data(vec![0x10, 0x00, 0x01]); // address to which init data is written
    sys_ex.data_u8(9); // reverb type 1-8, 9=off
    sys_ex.data_u8(1); // reverb time 1-8
    sys_ex.data_u8(0); // reverb level 0-7

    sys_ex.data(RES_ALLOWANCE_FOR_PARTIALS.to_vec());
    sys_ex.data(MIDI_CHANNELS.iter().map(|c| c - 1).collect::<Vec<u8>>());
    sys_ex.data_u8(0);
    sys_ex.data_str(&today());

    sys_ex
}


pub struct D110TimbreSysEx {
    pos: usize,
    pub data: [u8; 24]
}

pub struct D110ToneSysEx {
    pos: usize,
    pub data: [u8; 256]
}

