//! 1:1 port of `TestMasterMultiCommandRequests.cpp` (3 test cases).

use opendnp3::app::{AnalogOutputInt16, ControlRelayOutputBlock, Indexed};
use opendnp3::enums::{CommandStatus, OperationType, TaskCompletion, TripCloseCode};
use opendnp3::master::command::{CommandHeader, CommandSet};

use crate::common::{MasterFixture, no_startup_tasks};

fn pulse_on_crob() -> ControlRelayOutputBlock {
    ControlRelayOutputBlock::new(
        OperationType::PulseOn,
        TripCloseCode::Nul,
        false,
        1,
        100,
        100,
        CommandStatus::Success,
    )
}

#[test]
fn command_set_ignores_empty_headers() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();

    let commands = CommandSet {
        headers: vec![
            CommandHeader::Crob(Vec::new()),
            CommandHeader::Crob(vec![Indexed::new(1, pulse_on_crob())]),
        ],
    };
    t.master
        .direct_operate(commands, false, t.clock.now(), &mut t.app);

    let req = t.pop_write();
    assert!(req.ends_with(" 05 0C 01 28 01 00 01 00 01 01 64 00 00 00 64 00 00 00 00"));
}

#[test]
fn direct_operate_two_crob() {
    let crobstr = "0C 01 28 02 00 01 00 01 01 64 00 00 00 64 00 00 00 00 07 00 01 01 64 00 00 00 64 00 00 00 00";

    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();

    let commands = CommandSet {
        headers: vec![CommandHeader::Crob(vec![
            Indexed::new(1, pulse_on_crob()),
            Indexed::new(7, pulse_on_crob()),
        ])],
    };
    t.master
        .direct_operate(commands, false, t.clock.now(), &mut t.app);

    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert_eq!(req, format!("{seq} 05 {crobstr}"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00 {crobstr}"));

    assert!(t.pop_write().is_empty());
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}

#[test]
fn select_and_operate_two_crobs_one_ao() {
    let crobstr = "0C 01 28 02 00 01 00 01 01 64 00 00 00 64 00 00 00 00 07 00 01 01 64 00 00 00 64 00 00 00 00";
    let aostr = "29 02 28 01 00 08 00 34 12 00";
    let headers = format!("{crobstr} {aostr}");

    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();

    let commands = CommandSet {
        headers: vec![
            CommandHeader::Crob(vec![
                Indexed::new(1, pulse_on_crob()),
                Indexed::new(7, pulse_on_crob()),
            ]),
            CommandHeader::AnalogOutputInt16(vec![Indexed::new(8, AnalogOutputInt16::new(0x1234))]),
        ],
    };
    t.master
        .select_and_operate(commands, t.clock.now(), &mut t.app);

    let req1 = t.pop_write();
    let seq1 = req1[..2].to_string();
    assert_eq!(req1, format!("{seq1} 03 {headers}"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq1} 81 00 00 {headers}"));

    let req2 = t.pop_write();
    let seq2 = req2[..2].to_string();
    assert_eq!(req2, format!("{seq2} 04 {headers}"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq2} 81 00 00 {headers}"));

    assert!(t.pop_write().is_empty());
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}
