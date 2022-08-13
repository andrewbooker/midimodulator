use std::{
    os::raw::{c_char, c_uchar, c_int, c_uint, c_void},
    ptr,
    thread,
    time::Duration,
    ffi::{CStr}
};

#[repr(C)]
pub struct PmDeviceInfo {
    pub struct_version: c_int,
    pub interf: *const c_char,
    pub name: *const c_char,
    pub input: c_int,
    pub output: c_int,
    pub opened: c_int,
}

#[allow(dead_code)]
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

#[repr(C)]
pub struct C_PmEvent {
    pub message: i32,
    pub timestamp: u32,
}

#[allow(dead_code)]
#[link(name = "portmidi")]
extern "C" {
    pub fn Pm_Initialize() -> c_int;
    pub fn Pm_Terminate() -> c_int;
    pub fn Pm_CountDevices() -> c_int;
    pub fn Pm_GetDeviceInfo(id: c_int) -> *const PmDeviceInfo;
    pub fn Pm_OpenOutput(stream: *const *const c_void, outputDeviceId: c_int, inputDriverInfo: *const c_void, bufferSize: i32, time_proc: *const c_void, time_info: *const c_void, latency: i32) -> PmError;
    pub fn Pm_OpenInput(stream: *const *const c_void, inputDevice: c_int, inputDriverInfo: *const c_void, bufferSize : i32, time_proc: *const c_void, time_info: *const c_void) -> PmError;
    pub fn Pm_WriteShort(stream: *const c_void, timestamp: u32, message: c_uint) -> PmError;
    pub fn Pm_Poll(stream : *const c_void) -> PmError;
    pub fn Pm_Read(stream: *const c_void, buffer: *mut C_PmEvent, length: i32) -> i16;
    pub fn Pm_Close(stream: *const c_void) -> PmError;
    pub fn Pm_WriteSysEx(stream: *const c_void, when: u32, msg: *const c_uchar) -> PmError;
}

pub struct MidiMessage {
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
    data3: u8
}

impl MidiMessage {
    pub fn note_on(note: u8, channel: u8) -> MidiMessage {
        MidiMessage { status: 0x90 | channel, data1: note, data2: 100, data3: 0 }
    }
    pub fn note_off(note: u8, channel: u8) -> MidiMessage {
        MidiMessage { status: 0x80 | channel, data1: note, data2: 0, data3: 0 }
    }
    pub fn program(p: u8, channel: u8) -> MidiMessage {
        MidiMessage { status: 0xC0 | channel, data1: p, data2: 0, data3: 0 }
    }
    pub fn as_u32(&self) -> u32 {
        (self.data3 as u32) << 24
            | (self.data2 as u32) << 16
            | (self.data1 as u32) << 8
            | self.status as u32
    }
}

fn to_string(s: *const c_char) -> String {
    unsafe { CStr::from_ptr(s) }.to_str().ok().unwrap().to_owned()
}

pub struct MidiOutDevices;
impl MidiOutDevices {
    pub fn list() {
        let n = unsafe { Pm_CountDevices() };
        for d in 0..n {
            let info_ptr = unsafe { Pm_GetDeviceInfo(d) };
            if 1 == unsafe { (*info_ptr).output } {
                println!("{} {} can output", d, to_string(unsafe { (*info_ptr).name }));
            }
        }
    }
}

pub struct MidiOut {
    ostream: *const c_void
}

impl MidiOut {
    pub fn using_device(id: i32) -> MidiOut {
        unsafe { Pm_Initialize() };
        let m = MidiOut {
            ostream: ptr::null()
        };
        let buffer_size: c_int = 1024;
        let res = unsafe { Pm_OpenOutput(&m.ostream, id, ptr::null(), buffer_size, ptr::null(), ptr::null(), 0) };
        println!("opening output: {}", res as i32);
        thread::sleep(Duration::from_millis(1000));
        m
    }

    pub fn send(&mut self, m: &MidiMessage) {
        unsafe { Pm_WriteShort(self.ostream, 0, m.as_u32()) };
    }

    pub fn send_sys_ex(&mut self, data: &[u8]) {
        let res = unsafe { Pm_WriteSysEx(self.ostream, 0, data.as_ptr()) };
        println!("sys_ex: {}", res as i32);
        thread::sleep(Duration::from_millis(100));
    }
}

impl Drop for MidiOut {
    fn drop(&mut self) {
        unsafe { Pm_Close(self.ostream) };
        unsafe { Pm_Terminate() };
        println!("MidiOut closed");
    }
}


pub struct MidiInDevices;
impl MidiInDevices {
    pub fn list() {
        let n = unsafe { Pm_CountDevices() };
        for d in 0..n {
            let info_ptr = unsafe { Pm_GetDeviceInfo(d) };
            if 1 == unsafe { (*info_ptr).input } {
                println!("{} {} can input", d, to_string(unsafe { (*info_ptr).name }));
            }
        }
    }
}

pub struct MidiIn {
    istream: *const c_void
}

pub trait MidiCallback {
    fn receive(&mut self, message: &MidiMessage);
}

impl MidiIn {
    pub fn using_device(id: i32) -> MidiIn {
        let m = MidiIn {
            istream: ptr::null()
        };
        let buffer_size: c_int = 1024;
        let res = unsafe { Pm_OpenInput(&m.istream, id, ptr::null(), buffer_size, ptr::null(), ptr::null()) };
        println!("opening input: {}", res as i32);
        thread::sleep(Duration::from_millis(1000));
        m
    }

    pub fn read<C: MidiCallback>(&mut self, callback: &mut C) {
        let status: PmError = unsafe { Pm_Poll(self.istream) };
        match status as PmError {
            PmError::PmGotData => {
                let mut e = C_PmEvent {
                    message: 0,
                    timestamp: 0
                };
                let len: i16 = unsafe { Pm_Read(self.istream, &mut e, 1) };
                if len > 0 {
                    let msg = MidiMessage {
                        data3: 0,
                        data2: (e.message >> 16) as u8,
                        data1: (e.message >> 8) as u8,
                        status: (e.message & 0xFF) as u8
                    };
                    callback.receive(&msg);
                }
            },
            _ => {}
        }
    }
}

impl Drop for MidiIn {
    fn drop(&mut self) {
        unsafe { Pm_Close(self.istream) };
        unsafe { Pm_Terminate() };
        println!("MidiIn closed");
    }
}
