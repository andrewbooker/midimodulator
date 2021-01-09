extern crate libc;
mod korg;
use crate::korg::{CHANNEL, KorgProgramSysEx};

use std::{
    f32,
    thread,
    time::{Duration, Instant},
    os::raw::{c_char, c_uchar, c_int, c_uint, c_void},
    ptr,
    ffi::{CStr}
};


struct ModulationProfile {
    freq_hz: f32,
    min_val: i32,
    max_val: i32,

    previous_val: i32,
    current_val: i32
}

impl ModulationProfile {
    fn new(f_hz: f32, min_v: i32, max_v: i32) -> ModulationProfile {
        ModulationProfile {
            freq_hz: f_hz,
            min_val: min_v,
            max_val: max_v,
            current_val: max_v,
            previous_val: max_v
        }
    }

    fn update(&mut self, since_start: &Instant) {
        let dt = since_start.elapsed().as_millis() as f32;
        let ang_freq = self.freq_hz * 2.0 * f32::consts::PI as f32;

        self.previous_val = self.current_val;
        let val = self.min_val as f32 + ((self.max_val - self.min_val) as f32 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()));
        self.current_val = val.round() as i32;
    }
}


#[repr(C)]
pub struct PmDeviceInfo {
    pub struct_version: c_int,
    pub interf: *const c_char,
    pub name: *const c_char,
    pub input: c_int,
    pub output: c_int,
    pub opened: c_int,
}

#[repr(C)]
pub enum PmError {
    PmNoError = 0,
    PmGotData = 1,
    PmHostError = -10000,
    PmInvalidDeviceId = -9999,
    PmInsufficientMemory = -9998,
    PmBufferTooSmall = -9997,
    PmBufferOverflow = -9996,
    PmBadPtr = -9995, // stream is null or not opened or input/output direction mismatch
    PmBadData = -9994, // e.g. missing EOX
    PmInternalError = -9993,
    PmBufferMaxSize = -9992,
}


#[link(name = "portmidi")]
extern "C" {
    pub fn Pm_Initialize() -> c_int;
    pub fn Pm_Terminate() -> c_int;
    pub fn Pm_CountDevices() -> c_int;
    pub fn Pm_GetDeviceInfo(id: c_int) -> *const PmDeviceInfo;
    pub fn Pm_OpenOutput(stream: *const *const c_void, outputDeviceId: c_int, inputDriverInfo: *const c_void, bufferSize: i32, time_proc: *const c_void, time_info: *const c_void, latency: i32) -> PmError;
    pub fn Pm_WriteShort(stream: *const c_void, timestamp: u32, message: c_uint) -> PmError;
    pub fn Pm_Close(stream: *const c_void) -> PmError;
    pub fn Pm_WriteSysEx(stream: *const c_void, when: u32, msg: *const c_uchar) -> PmError;
}

struct MidiMessage {
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
    pub data3: u8
}



impl MidiMessage {
    fn note_on(note: u8) -> MidiMessage {
        MidiMessage { status: 0x90 + CHANNEL, data1: note, data2: 100, data3: 0 }
    }
    fn note_off(note: u8) -> MidiMessage {
        MidiMessage { status: 0x80 + CHANNEL, data1: note, data2: 0, data3: 0 }
    }
    fn program(p: u8) -> MidiMessage {
        MidiMessage { status: 0xC0 + CHANNEL, data1: p, data2: 0, data3: 0 }
    }
    fn as_u32(&self) -> u32 {
        (self.data3 as u32) << 24
            | (self.data2 as u32) << 16
            | (self.data1 as u32) << 8
            | self.status as u32
    }
}


fn to_string(s: *const c_char) -> String {
    unsafe { CStr::from_ptr(s) }.to_str().ok().unwrap().to_owned()
}





fn build_prog_sys_ex(psx: &mut KorgProgramSysEx) {
    psx
        .name("2021-01-05")
        .data(1) // osc: double
        .data(0) // bit0: poly/mono, bit1: hold off/on
        .data_double_byte(12) // osc1
        .data(0) // octave1: -2 ... 1 = 32,16,8,4
        .data_double_byte(13) // osc2
        .data(0) // octave2
        .data(0) // interval
    ;
}

struct KorgInitSysEx {
    data: [u8; 8]
}

impl KorgInitSysEx {
    fn new() -> KorgInitSysEx {
        KorgInitSysEx {
            data: [0xF0,
                   0x42, // ID of Korg
                   0x30 | CHANNEL, // format ID (3), channel
                   0x36, // 05R/W ID
                   0x4E, // mode change
                   0x03, // program edit
                   0x00,
                   0xF7]
        }
    }
}


fn main() {
    let start = Instant::now();
    let mut mp = ModulationProfile::new(0.05, -50, 40);

    for i in 0..10 {
        mp.update(&start);
        println!("{} {} {}", i, mp.current_val, mp.previous_val);
        thread::sleep(Duration::from_millis(100));
    }

    unsafe { Pm_Initialize() };
    let c = unsafe { Pm_CountDevices() };
    println!("{} devices found", c);
    let device_id: c_int = 2;

    let info_ptr = unsafe { Pm_GetDeviceInfo(device_id) };
    println!("{}", unsafe { (*info_ptr).output });
    println!("using {}", to_string(unsafe { (*info_ptr).name }));

    let ostream: *const c_void = ptr::null();
    let buffer_size: c_int = 1024;
    let res = unsafe { Pm_OpenOutput(&ostream, device_id, ptr::null(), buffer_size, ptr::null(), ptr::null(), 0) };
    println!("opening output: {}", res as i32);
    thread::sleep(Duration::from_millis(1000));

    let prog28 = MidiMessage::program(33);
    let res_prog28 = unsafe { Pm_WriteShort(ostream, 0, prog28.as_u32()) };
    println!("prog change {:x} gave {}", prog28.as_u32(), res_prog28 as i32);
    thread::sleep(Duration::from_millis(1000));

    let kssx = KorgInitSysEx::new();
    let sysex_res = unsafe { Pm_WriteSysEx(ostream, 0, kssx.data.as_ptr()) };
    println!("sys_ex: {}", sysex_res as i32);
    println!("{:?}", kssx.data);
    thread::sleep(Duration::from_millis(1000));

    let note = 67;
    let on = MidiMessage::note_on(note);
    let off = MidiMessage::note_off(note);

    let res_on = unsafe { Pm_WriteShort(ostream, 0, on.as_u32()) };
    println!("{:x} gave {}", on.as_u32(), res_on as i32);
    thread::sleep(Duration::from_millis(2000));
    let res_off = unsafe { Pm_WriteShort(ostream, 0, off.as_u32()) };
    println!("{:x} gave {}", off.as_u32(), res_off as i32);
    thread::sleep(Duration::from_millis(1000));

    unsafe { Pm_Close(ostream) };
    unsafe { Pm_Terminate() };

    let mut kpsx = KorgProgramSysEx::new();
    build_prog_sys_ex(&mut kpsx);

    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }
    let mut port = serialport::new("/dev/ttyUSB0", 38400)
                    .timeout(Duration::from_millis(1000))
                    .open()
                    .expect("Failed to open port");

    port.write(&kpsx.data).expect("Write failed!");
}
