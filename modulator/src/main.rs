extern crate libc;

mod korg;
mod midi;
mod utils;
mod modulation;

use crate::modulation::{
    SysExComposer,
    PairedUpdater,
    StepInterval,
    SweepState,
    Selector
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
    time::{Duration, Instant},
    sync::{mpsc, mpsc::{Sender, Receiver}},
    io::prelude::*,
    collections::HashMap
};



struct TimeBasedInterval {
    start: Instant
}

impl TimeBasedInterval {
    fn new() -> TimeBasedInterval {
        TimeBasedInterval {
            start: Instant::now()
        }
    }
}

impl StepInterval for TimeBasedInterval {
    fn interval(&self) -> f32 {
        self.start.elapsed().as_millis() as f32
    }
}


fn modulate_korg<C>(cmd_dump_rx: &Receiver<C>, res_tx: &Sender<HashMap<std::string::String, SweepState>>, edit_tx: &Sender<i32>) {
    let mut port = serialport::new("/dev/ttyUSB0", 38400)
                    .timeout(Duration::from_millis(1000))
                    .open()
                    .expect("Failed to open port");

    let interval = TimeBasedInterval::new();
    let mut updater = PairedUpdater::new(&interval);
    let mut effect_selector = KorgEffectSelector::new();
    let mut osc_selector = KorgOscSelector::new();

    let today = utils::today();
    let mut first = true;
    let mut osc1 = 0;
    let mut osc2 = 0;

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
        updater.sweep_alternator();
        let send_edit = osc1 != osc_selector.val(0) || osc2 != osc_selector.val(1);
        if send_edit {
            edit_tx.send(1).unwrap();
            osc1 = osc_selector.val(0);
            osc2 = osc_selector.val(1);
        }

        port.write(&kpsx.data).expect("Write failed!");
        thread::sleep(Duration::from_millis(100));

        match cmd_dump_rx.try_recv() {
            Ok(_) => {
                res_tx.send(updater.sweep_state.clone()).unwrap();
            },
            _ => {}
        }
        if first || send_edit {
            edit_tx.send(0).unwrap();
            first = false;
        }
    }
}


fn main() {
    let korg_number = MidiOutDevices::index_of("USB Midi").unwrap();
    println!("Korg port {}", korg_number);

    //thread::spawn(move || { receive_play_notifications(d110_number); });

    let mut midi_out = MidiOut::using_device(korg_number);
    midi_out.send_sys_ex(&KorgInitSysEx::new(0x02).data); // select prog
    midi_out.send(&MidiMessage::program(33, korg::CHANNEL)); // select 33
    thread::sleep(Duration::from_millis(100));

    midi_out.send_sys_ex(&KorgInitSysEx::new(0x03).data); // edit prog
    midi_out.send_sys_ex(&KorgSingleParamSysEx::new(0, 1).data); // oscillator mode: Double, on UI, otherwise the screen value overrides th sysEx

    let (cmd_dump_tx, cmd_dump_rx) = mpsc::channel();
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();
    let (res_tx, res_rx) = mpsc::channel();
    let (edit_tx, edit_korg_rx) = mpsc::channel();

    thread::spawn(move || { modulate_korg(&cmd_dump_rx, &res_tx, &edit_tx); });

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
        match edit_korg_rx.try_recv() {
            Ok(0) => {
                //midi_out.send_sys_ex(&KorgInitSysEx::new(0x02).data); // select prog deselect edit
                println!("edit off");
            },
            Ok(1) => {
                //midi_out.send_sys_ex(&KorgInitSysEx::new(0x03).data); // select prog edit
                println!("edit on");
            },
            _ => {}
        }
        thread::sleep(Duration::from_millis(100));
    }
    thread::sleep(Duration::from_millis(2000));
}
