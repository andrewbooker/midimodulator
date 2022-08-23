pub const CHANNEL_D110: u8 = 9;

pub struct D110InitSysEx {
    pos: usize,
    sum: u32,
    data: [u8; 35]
}

const RES_ALLOWANCE_FOR_PARTIALS: [u8; 9] = [32, 0, 0, 0, 0, 0, 0, 0, 0];
const MIDI_CHANNELS: [u8; 9] = [16, 9, 9, 9, 9, 9, 9, 9, 1];


impl D110InitSysEx {
    fn new() -> D110InitSysEx {
        D110InitSysEx {
            pos: 0,
            sum: 0,
            data: [0; 35]
        }
    }

    fn data_u8(&mut self, v: u8) {
        self.data[self.pos] = v;
        self.sum += v as u32;
        self.pos += 1;
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
        if self.pos < 35 {
            println!("too few bytes");
        }
        128 - (self.sum % 128)
    }

    pub fn to_send(&self) -> [u8; 46] {
        let mut msg: [u8; 46] = [0; 46];

        msg[0] = 0xF0;
        msg[1] = 0x41; // ID of Roland
        msg[2] = 0x10; // device
        msg[3] = 0x16; // model
        msg[4] = 0x12; // command

        msg[5..(5 + 35)].copy_from_slice(&self.data[0..]);

        msg[44] = self.checksum() as u8;
        msg[45] = 0xF7;

        println!("length: {}", self.pos);
        msg
    }
}

pub fn init_d110() -> D110InitSysEx {
    let mut sys_ex = D110InitSysEx::new();

    sys_ex.data(vec![0x10, 0x00, 0x01]); // address to which init data is written
    sys_ex.data_u8(9); // reverb type 1-8, 9=off
    sys_ex.data_u8(1); // reverb time 1-8
    sys_ex.data_u8(0); // reverb level 0-7

    sys_ex.data(RES_ALLOWANCE_FOR_PARTIALS.to_vec());
    sys_ex.data(MIDI_CHANNELS.iter().map(|c| c - 1).collect::<Vec<u8>>());
    sys_ex.data_u8(0);
    sys_ex.data_str("2022 08 24");

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

