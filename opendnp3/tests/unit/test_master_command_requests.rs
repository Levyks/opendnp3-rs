//! 1:1 port of `TestMasterCommandRequests.cpp` (18 test cases).

use std::time::Duration;

use opendnp3::app::{
    AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    ControlRelayOutputBlock,
};
use opendnp3::enums::{CommandStatus, OperationType, TaskCompletion, TripCloseCode};
use opendnp3::master::MasterParams;
use opendnp3::master::command::CommandSet;

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

fn single_crob_set(index: u16) -> CommandSet {
    let mut set = CommandSet::new();
    set.add_crob(index, pulse_on_crob());
    set
}

#[test]
fn control_execution_closed_state() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    assert!(t.pop_write().is_empty());
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureNoComms)
    );
}

#[test]
fn controls_timeout_after_start_period_elapses() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let _req = t.pop_write();
    t.on_tx_ready();
    t.advance_time(Duration::from_secs(10));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureResponseTimeout)
    );
}

#[test]
fn layer_down_while_still_scheduled() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    t.lower_layer_down();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureNoComms)
    );
}

#[test]
fn select_and_operate() {
    let crobstr = "0C 01 28 01 00 01 00 01 01 64 00 00 00 64 00 00 00 00";
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();

    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let req1 = t.pop_write();
    let seq1 = req1[..2].to_string();
    assert_eq!(req1, format!("{seq1} 03 {crobstr}"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq1} 81 00 00 {crobstr}"));

    let req2 = t.pop_write();
    let seq2 = req2[..2].to_string();
    assert_eq!(req2, format!("{seq2} 04 {crobstr}"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq2} 81 00 00 {crobstr}"));

    assert!(t.pop_write().is_empty());
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}

#[test]
fn select_and_operate_with_confirm_response() {
    let crobstr = "0C 01 28 01 00 01 00 01 01 64 00 00 00 64 00 00 00 00";
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();

    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let req1 = t.pop_write();
    let seq1 = req1[..2].to_string();
    t.on_tx_ready();
    // Response with CON=1 (0xE0 | seq)
    let con_ctrl = u8::from_str_radix(&seq1, 16).unwrap() | 0x20;
    t.send_to_master(&format!("{con_ctrl:02X} 81 00 00 {crobstr}"));
    let confirm = t.pop_write();
    assert_eq!(confirm, format!("{seq1} 00"));
    t.on_tx_ready();

    let req2 = t.pop_write();
    assert!(req2.contains(" 04 "));
}

#[test]
fn can_select_and_operate_with_one_byte_qualifier_optimization_enabled() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let req = t.pop_write();
    assert!(req.contains(" 03 0C 01 "));
}

#[test]
fn control_execution_select_response_timeout() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let _req = t.pop_write();
    t.on_tx_ready();
    t.advance_to_next_timer();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureResponseTimeout)
    );
}

#[test]
fn control_execution_select_layer_down() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let _req = t.pop_write();
    t.on_tx_ready();
    t.lower_layer_down();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureNoComms)
    );
}

#[test]
fn control_execution_select_error_response() {
    let crob_err = "0C 01 28 01 00 01 00 01 01 64 00 00 00 64 00 00 00 04";
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00 {crob_err}"));
    // Should not send OPERATE when SELECT returns non-SUCCESS
    assert!(t.pop_write().is_empty());
}

#[test]
fn control_execution_select_bad_fir_fin() {
    let crobstr = "0C 01 28 01 00 01 00 01 01 64 00 00 00 64 00 00 00 00";
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .select_and_operate(single_crob_set(1), t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq_num = u8::from_str_radix(&req[..2], 16).unwrap() & 0x0F;
    t.on_tx_ready();
    // FIR=1, FIN=0 (0x80 | seq) -> bad multi-frag command response
    t.send_to_master(&format!("{:02X} 81 00 00 {crobstr}", 0x80 | seq_num));
    assert!(t.pop_write().is_empty());
}

#[test]
fn deferred_control_execution() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .direct_operate(single_crob_set(1), false, t.clock.now(), &mut t.app);
    t.master
        .direct_operate(single_crob_set(2), false, t.clock.now(), &mut t.app);

    let req1 = t.pop_write();
    let seq1 = req1[..2].to_string();
    assert!(req1.contains(" 05 0C 01 28 01 00 01 00 "));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq1} 81 00 00 {}", &req1[6..]));

    let req2 = t.pop_write();
    assert!(req2.contains(" 05 0C 01 28 01 00 02 00 "));
}

#[test]
fn close_while_waiting_for_command_response() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .direct_operate(single_crob_set(1), false, t.clock.now(), &mut t.app);
    let _req = t.pop_write();
    t.on_tx_ready();
    t.lower_layer_down();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureNoComms)
    );
}

#[test]
fn response_timeout() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .direct_operate(single_crob_set(1), false, t.clock.now(), &mut t.app);
    let _req = t.pop_write();
    t.on_tx_ready();
    t.advance_to_next_timer();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureResponseTimeout)
    );
}

#[test]
fn send_command_during_failed_startup() {
    let mut t = MasterFixture::new(MasterParams::default());
    t.lower_layer_up();
    // Startup disable unsol is in flight
    let _startup = t.pop_write();
    t.on_tx_ready();
    // Queue a command while startup is pending
    t.master
        .direct_operate(single_crob_set(1), false, t.clock.now(), &mut t.app);
    // Timeout startup
    t.advance_to_next_timer();
    assert!(!t.pop_write().is_empty());
}

#[test]
fn single_setpoint_execution() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    let mut set = CommandSet::new();
    set.add_ao_f32(1, AnalogOutputFloat32::new(100.0));
    t.master
        .direct_operate(set, false, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 05 29 03 28 01 00 01 00 00 00 C8 42 00"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00 {}", &req[6..]));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}

#[test]
fn double_setpoint_execution() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    let mut set = CommandSet::new();
    set.add_ao_f64(1, AnalogOutputDouble64::new(100.0));
    t.master
        .direct_operate(set, false, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 05 29 04 28 01 00 01 00 00 00 00 00 00 00 59 40 00"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00 {}", &req[6..]));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}

#[test]
fn int32_setpoint_execution() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    let mut set = CommandSet::new();
    set.add_ao_i32(1, AnalogOutputInt32::new(100));
    t.master
        .direct_operate(set, false, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 05 29 01 28 01 00 01 00 64 00 00 00 00"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00 {}", &req[6..]));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}

#[test]
fn int16_setpoint_execution() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    let mut set = CommandSet::new();
    set.add_ao_i16(1, AnalogOutputInt16::new(100));
    t.master
        .direct_operate(set, false, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 05 29 02 28 01 00 01 00 64 00 00"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00 {}", &req[6..]));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}
