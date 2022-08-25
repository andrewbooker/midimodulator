
use crate::utils::today;

use crate::modulation::{
    SysExComposer,
    Updater
};


//pub const CHANNEL_D110: u8 = 9;


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

    fn data_vec_u8(&mut self, values: Vec<u8>) {
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

impl SysExComposer for D110SysEx {
    fn data(&mut self, d: i8) {
        self.data_u8(d as u8);
    }
    fn data_double_byte(&mut self, _: i16) {}
    fn name(&mut self, _: &str) {}
}


pub fn init_d110() -> D110SysEx {
    const RES_ALLOWANCE_FOR_PARTIALS: [u8; 9] = [32, 0, 0, 0, 0, 0, 0, 0, 0];
    const MIDI_CHANNELS: [u8; 9] = [16, 9, 9, 9, 9, 9, 9, 9, 1];

    let mut sys_ex = D110SysEx::new();

    sys_ex.data_vec_u8(vec![0x10, 0x00, 0x01]); // address to which init data is written
    sys_ex.data_u8(9); // reverb type 1-8, 9=off
    sys_ex.data_u8(1); // reverb time 1-8
    sys_ex.data_u8(0); // reverb level 0-7

    sys_ex.data_vec_u8(RES_ALLOWANCE_FOR_PARTIALS.to_vec());
    sys_ex.data_vec_u8(MIDI_CHANNELS.iter().map(|c| c - 1).collect::<Vec<u8>>());
    sys_ex.data_u8(0);
    sys_ex.data_str(&today());

    sys_ex
}

pub fn init_timbre(number: u8) -> D110SysEx {
    let mut sys_ex = D110SysEx::new();
    
    sys_ex.data_vec_u8(vec![0x03, 0x00, 0x02 + (0x10 * (number - 1))]); // address
    sys_ex.data_u8(24); // keyShift in semitones, 24 = 0 shift, 27 = +3
    sys_ex.data_u8(50); // fineTune +/- 50, 50 = 0
    sys_ex.data_u8(12); // benderRange semitones, 0-24
    sys_ex.data_u8(2); // note priority monoLast = 0, monoFirst, polyLast, polyFirst
    sys_ex.data_u8(1); // outputAssign 1=mix?
    sys_ex.data_u8(0); // reverb off
    sys_ex.data_u8(if number == 1 { 98 } else { 0 });  // outputLevel max 100
    sys_ex.data_u8(7);  // pan 7 = mid, 0 = R, 15 = L
    sys_ex.data_u8(if number == 1 { 0 } else { 0x7F }); // keyRangeLower 0 = C-1
    sys_ex.data_u8(0x7F); // keyRangeUpper 127 = G9
    sys_ex.data_u8(0);
    sys_ex.data_u8(0);
    sys_ex.data_u8(0);
    sys_ex.data_u8(0);
    
    sys_ex
}


fn address_of(part_number: u8) -> u32 {
    if part_number == 1 {
        0x040000
    } else {
        0x040176 + (502 * (part_number as u32 - 2))
    }
}

pub fn set_up_part(number: u8) -> D110SysEx {
    let mut sys_ex = D110SysEx::new();

    let a = address_of(number);
    let a_vec = vec![(a >> 16) as u8, ((a >> 8) & 0xFF) as u8, (a & 0xFF) as u8];

    sys_ex.data_vec_u8(a_vec); // address
    sys_ex.data_str(if number == 1 { "part" } else { "mute" });
    sys_ex.data_u8(number + 0x30);
    sys_ex.data_vec_u8([0x20; 5].to_vec());
    sys_ex.data_u8(0); // 0 = ss, 5 = pp
    sys_ex.data_u8(5); // 0 = ss, 5 = pp
    sys_ex.data_u8(if number == 1 { 0xF } else { 0 });
    sys_ex.data_u8(0); // envelope mode
    sys_ex
}

// typedef enum t_partialConfig { ss = 0, ss_r, ps, ps_r, sp_r, pp, pp_r, s_s, p_p, ss_r_noDry, ps_r_noDry, sp_r_noDry, pp_r_noDry };


pub const PARTIAL_SPEC: [Updater; 4] = [
    Updater::Const("pitchCoarse", 36),
    Updater::Sweep("pitchFine", 40, 60), // 0-100 -> +/- 50
    Updater::Const("keyFollowPitch", 11),
    Updater::Const("allowPitchBend", 1)
];
