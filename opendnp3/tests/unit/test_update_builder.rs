//! 1:1 port of `TestUpdateBuilder.cpp` (1 test case).

use opendnp3::app::Counter;
use opendnp3::enums::EventMode;
use opendnp3::outstation::{Database, DatabaseConfig, EventBufferConfig};

type UpdateOp = Box<dyn FnOnce(&mut Database)>;

/// Minimal update batch builder mirroring C++ `UpdateBuilder::Build()` clearing semantics.
#[derive(Default)]
struct UpdateBuilder {
    ops: Vec<UpdateOp>,
}

impl UpdateBuilder {
    fn update_counter(&mut self, value: Counter, index: u16) {
        self.ops.push(Box::new(move |db| {
            db.update_counter(index, value, EventMode::Detect);
        }));
    }

    fn build(&mut self) -> Vec<UpdateOp> {
        std::mem::take(&mut self.ops)
    }
}

#[test]
fn builder_is_cleared_after_building() {
    let mut builder = UpdateBuilder::default();
    builder.update_counter(Counter::new(42), 0);

    let updates = builder.build();
    assert!(!updates.is_empty());

    let mut db = Database::new(
        &DatabaseConfig::all_types(1),
        EventBufferConfig::all_types(10),
    );
    for op in updates {
        op(&mut db);
    }

    let updates2 = builder.build();
    assert!(updates2.is_empty());
}
