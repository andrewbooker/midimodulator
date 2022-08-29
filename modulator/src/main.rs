extern crate libc;

mod korg;
mod midi;
mod d110;
mod utils;
mod modulation;

use crate::modulation::{
    SysExComposer,
    PairedUpdater
};
use crate::d110::{
    init_d110,
    init_timbre,
    set_up_part,
    PARTIAL_SPEC
};
use crate::korg::{
    KorgProgramSysEx,
    KorgInitSysEx,
    KorgSingleParamSysEx,
    PROGRAM_SPEC,
    OSC_SPEC,
    KorgOscSelector,
    KorgEffectSelector
};
use crate::midi::{MidiMessage, MidiOut, MidiOutDevices};
use std::{
    thread,
    time::Duration,
    sync::mpsc
};


use crate::modulation::Selector;

struct DummySelector;

impl DummySelector {
    fn new() -> DummySelector {
        DummySelector {}
    }
}

impl Selector for DummySelector {
    fn next1(&mut self) {}
    fn next2(&mut self) {}

    fn val(&self, _: u8) -> u16 { 0 }
}


fn main() {
    MidiOutDevices::list();

    let mut midi_out = MidiOut::using_device(4);
    {
        let kssx = KorgInitSysEx::new(0x02); // select prog
        midi_out.send_sys_ex(&kssx.data);
    }

    midi_out.send(&MidiMessage::program(33, korg::CHANNEL));
    thread::sleep(Duration::from_millis(100));

    {
        let kssx = KorgInitSysEx::new(0x03); // edit prog
        midi_out.send_sys_ex(&kssx.data);
    }

    {
        let kssx = KorgSingleParamSysEx::new(0, 1); // oscillator mode: Double, on UI, otherwise the screen value overrides th sysEx
        midi_out.send_sys_ex(&kssx.data);
    }

    {
        let mut d110_midi_out = MidiOut::using_device(2);
        let d110_init = init_d110();
        d110_midi_out.send_sys_ex(&d110_init.to_send());
        for t in 1..9 {
            println!("sending timbre {}", t);
            d110_midi_out.send_sys_ex(&init_timbre(t).to_send());
        }
        for t in 2..9 {
            println!("muting part {}", t);
            d110_midi_out.send_sys_ex(&set_up_part(t).to_send());
        }
        println!("D110 init sent");

        let mut p1 = set_up_part(1);
        let mut updater = PairedUpdater::new();
        let mut dummy_1 = DummySelector::new();
        let mut dummy_2 = DummySelector::new();
        updater.update(&mut p1, &mut dummy_1, &mut dummy_2, &PARTIAL_SPEC, Some("partial1"));
        for (key, val) in &updater.sweep_state {
            println!("{}: {}", key, val.val);
        }
        d110_midi_out.send_sys_ex(&p1.to_send());

        println!("part1 updated");
    }

    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }

    let (cmd_dump_tx, cmd_dump_rx) = mpsc::channel();
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();
    let (res_tx, res_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut port = serialport::new("/dev/ttyUSB0", 38400)
                    .timeout(Duration::from_millis(1000))
                    .open()
                    .expect("Failed to open port");

        let mut updater = PairedUpdater::new();
        let mut effect_selector = KorgEffectSelector::new();
        let mut osc_selector = KorgOscSelector::new();

        let today = utils::today();

        loop {
            let mut kpsx = KorgProgramSysEx::new();
            kpsx.name(&today);
            
            let eff1_updater = &effect_selector.eff1.updater;
            let eff2_updater = &effect_selector.eff2.updater;
            let pre_eff = &effect_selector.pre_eff();

            updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, &PROGRAM_SPEC, None);
            updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, &OSC_SPEC, Some("osc1"));
            updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, &OSC_SPEC, Some("osc2"));
            updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, pre_eff, None);
            updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, eff1_updater, Some("eff1"));
            updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, eff2_updater, Some("eff2"));

            port.write(&kpsx.data).expect("Write failed!");
            thread::sleep(Duration::from_millis(100));

            match cmd_dump_rx.try_recv() {
                Ok(_) => {
                    res_tx.send(updater.sweep_state.clone()).unwrap();
                },
                _ => {}
            }
        }
    });

    thread::spawn(move || {
        let g = getch::Getch::new();
        loop {
            let c: u8 = g.getch().unwrap();
            match c as char {
                'l' => {
                    cmd_dump_tx.send(()).unwrap();
                    for res in &res_rx {
                        for (key, val) in &res {
                            println!("{}: {}", key, val.val);
                        }
                        break;
                    }
                },
                'q' => {
                    cmd_stop_tx.send(()).unwrap();
                    break;
                },
                _ => {}
            }
        }
    });

    loop {
        match cmd_stop_rx.try_recv() {
            Ok(_) => {
                println!("stopping...");
                break;
            },
            _ => {}
        }
    }
    thread::sleep(Duration::from_millis(2000));
}
