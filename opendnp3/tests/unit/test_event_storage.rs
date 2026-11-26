//! 1:1 port of `TestEventStorage.cpp` (7 test cases).

use opendnp3::app::{
    Analog, ApduHandler, ApduWriter, AppControlField, Binary, HeaderInfo, IinField, Indexed,
    ParseMode, ParseResult, parse_objects,
};
use opendnp3::enums::{
    EventAnalogVariation, EventBinaryVariation, EventClass, FunctionCode, GroupVariation,
};
use opendnp3::outstation::{EventBuffer, EventBufferConfig, EventPayload};

#[derive(Default)]
struct HeaderRecorder {
    headers: Vec<(GroupVariation, usize)>,
}

impl ApduHandler for HeaderRecorder {
    fn on_binaries(&mut self, info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        self.headers.push((info.gv, values.len()));
        IinField::empty()
    }
    fn on_analogs(&mut self, info: HeaderInfo, values: &[Indexed<Analog>]) -> IinField {
        self.headers.push((info.gv, values.len()));
        IinField::empty()
    }
}

fn write_and_record(storage: &mut EventBuffer) -> (usize, Vec<(GroupVariation, usize)>) {
    let mut w = ApduWriter::new_response(
        2048,
        AppControlField::request(0),
        FunctionCode::Response,
        IinField::empty(),
    );
    let _all_written = storage.write_selected(&mut w);
    let mut rec = HeaderRecorder::default();
    let (res, _) = parse_objects(&w.as_bytes()[4..], ParseMode::WithObjects, &mut rec);
    assert_eq!(res, ParseResult::Ok);
    let written: usize = rec.headers.iter().map(|(_, c)| *c).sum();
    (written, rec.headers)
}

#[test]
fn can_construct() {
    let storage = EventBuffer::new(EventBufferConfig::all_types(10));
    assert!(storage.is_empty());
}

#[test]
fn calls_write_multiple_times_for_different_variations() {
    let mut storage = EventBuffer::new(EventBufferConfig::all_types(10));
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Binary(Binary::new(true), EventBinaryVariation::Group2Var1),
    );
    assert!(!storage.is_overflown());
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Binary(Binary::new(true), EventBinaryVariation::Group2Var2),
    );
    assert!(!storage.is_overflown());

    assert_eq!(storage.select_by_class(EventClass::Ec1, usize::MAX), 2);
    assert_eq!(storage.num_selected(), 2);

    let (written, headers) = write_and_record(&mut storage);
    assert_eq!(written, 2);
    assert_eq!(storage.num_selected(), 0);
    assert_eq!(storage.num_unwritten(EventClass::Ec1), 0);
    assert_eq!(
        headers,
        vec![
            (GroupVariation::Group2Var1, 1),
            (GroupVariation::Group2Var2, 1),
        ]
    );
}

#[test]
fn calls_write_one_time_for_same_variation() {
    let mut storage = EventBuffer::new(EventBufferConfig::all_types(10));
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Binary(Binary::new(true), EventBinaryVariation::Group2Var1),
    );
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Binary(Binary::new(true), EventBinaryVariation::Group2Var1),
    );

    assert_eq!(storage.select_by_class(EventClass::Ec1, usize::MAX), 2);
    assert_eq!(storage.num_selected(), 2);
    assert_eq!(storage.num_unwritten(EventClass::Ec1), 2);

    let (written, headers) = write_and_record(&mut storage);
    assert_eq!(written, 2);
    assert_eq!(storage.num_selected(), 0);
    assert_eq!(storage.num_unwritten(EventClass::Ec1), 0);
    assert_eq!(headers, vec![(GroupVariation::Group2Var1, 2)]);
}

#[test]
fn calls_write_multiple_times_for_different_types() {
    let mut storage = EventBuffer::new(EventBufferConfig::all_types(10));
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Analog(Analog::new(1.0), EventAnalogVariation::Group32Var1),
    );
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Binary(Binary::new(true), EventBinaryVariation::Group2Var1),
    );
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Analog(Analog::new(1.0), EventAnalogVariation::Group32Var1),
    );

    assert_eq!(storage.select_by_class(EventClass::Ec1, usize::MAX), 3);

    let (written, headers) = write_and_record(&mut storage);
    assert_eq!(written, 3);
    assert_eq!(
        headers,
        vec![
            (GroupVariation::Group32Var1, 1),
            (GroupVariation::Group2Var1, 1),
            (GroupVariation::Group32Var1, 1),
        ]
    );
}

#[test]
fn zero_size_doesnt_overflow() {
    let mut storage = EventBuffer::new(EventBufferConfig::all_types(0));
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Analog(Analog::new(1.0), EventAnalogVariation::Group32Var1),
    );
    assert!(!storage.is_overflown());
    assert!(!storage.is_any_type_full());
    assert_eq!(storage.num_unwritten(EventClass::Ec1), 0);
    assert_eq!(storage.num_selected(), 0);
}

#[test]
fn overflows_as_expected() {
    let mut storage = EventBuffer::new(EventBufferConfig::all_types(1));
    assert_eq!(storage.num_unwritten(EventClass::Ec1), 0);
    assert!(!storage.is_any_type_full());

    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Analog(Analog::new(1.0), EventAnalogVariation::Group32Var1),
    );
    assert!(!storage.is_overflown());
    assert_eq!(storage.num_unwritten(EventClass::Ec1), 1);
    assert!(storage.is_any_type_full());
    assert_eq!(storage.num_selected(), 0);

    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Analog(Analog::new(1.0), EventAnalogVariation::Group32Var1),
    );
    assert!(storage.is_overflown());
    assert_eq!(storage.num_unwritten(EventClass::Ec1), 1);
}

#[test]
fn selected_events_discarded_on_overflow() {
    let mut storage = EventBuffer::new(EventBufferConfig::all_types(1));
    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Analog(Analog::new(1.0), EventAnalogVariation::Group32Var1),
    );
    assert_eq!(storage.select_by_class(EventClass::Ec1, usize::MAX), 1);

    storage.push(
        0,
        EventClass::Ec1,
        EventPayload::Analog(Analog::new(1.0), EventAnalogVariation::Group32Var1),
    );
    assert!(storage.is_overflown());

    let (written, headers) = write_and_record(&mut storage);
    assert_eq!(written, 0);
    assert!(headers.is_empty());
}
