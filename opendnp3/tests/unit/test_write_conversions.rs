//! 1:1 port of `TestWriteConversions.cpp` (3 test cases).

use opendnp3::app::{
    Analog, ApduWriter, AppControlField, IinField, ParseMode, ParseResult, parse_objects,
};
use opendnp3::enums::{AnalogQuality, FunctionCode, GroupVariation};

struct SingleAnalogHandler {
    value: Option<Analog>,
}

impl opendnp3::app::ApduHandler for SingleAnalogHandler {
    fn on_analogs(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<Analog>],
    ) -> IinField {
        if let Some(first) = values.first() {
            self.value = Some(first.value);
        }
        IinField::empty()
    }
}

fn roundtrip_g30v2(input: Analog) -> Analog {
    let mut w = ApduWriter::new_response(
        64,
        AppControlField::request(0),
        FunctionCode::Response,
        IinField::empty(),
    );
    assert!(w.write_range_header_u8(GroupVariation::Group30Var2, 0, 0));
    assert!(w.write_analog(GroupVariation::Group30Var2, &input));

    let mut handler = SingleAnalogHandler { value: None };
    let (res, _) = parse_objects(&w.as_bytes()[4..], ParseMode::WithObjects, &mut handler);
    assert_eq!(res, ParseResult::Ok);
    handler.value.expect("parsed analog")
}

#[test]
fn group30_var2_converts_within_range() {
    let a = Analog::new(12.0);
    let gv = roundtrip_g30v2(a);
    assert_eq!(gv.value, 12.0);
    assert_eq!(gv.flags.value, AnalogQuality::Online as u8);
}

#[test]
fn group30_var2_converts_overrange() {
    let a = Analog::new(32768.0);
    let gv = roundtrip_g30v2(a);
    assert_eq!(gv.value, 32767.0);
    assert_eq!(
        gv.flags.value,
        (AnalogQuality::Online as u8) | (AnalogQuality::OverRange as u8)
    );
}

#[test]
fn group30_var2_converts_underrange() {
    let a = Analog::new(-32769.0);
    let gv = roundtrip_g30v2(a);
    assert_eq!(gv.value, -32768.0);
    assert_eq!(
        gv.flags.value,
        (AnalogQuality::Online as u8) | (AnalogQuality::OverRange as u8)
    );
}
