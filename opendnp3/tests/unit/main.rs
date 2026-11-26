//! 1:1 port of the reference C++ opendnp3 unit test suite.
#![allow(clippy::field_reassign_with_default)]

mod common;
mod test_apdu_parsing;
mod test_apdu_writing;
mod test_control_relay_output_block;
mod test_crc;
mod test_database;
mod test_event_storage;
mod test_flags;
mod test_link_addresses;
mod test_link_frame;
mod test_link_layer;
mod test_link_layer_keep_alive;
mod test_link_receiver;
mod test_master;
mod test_master_assign_class;
mod test_master_command_requests;
mod test_master_multi_command_requests;
mod test_master_multidrop;
mod test_master_unsol_behaviors;
mod test_measurement_handler;
mod test_outstation;
mod test_outstation_assign_class;
mod test_outstation_broadcast;
mod test_outstation_command_responses;
mod test_outstation_discontiguous_indices;
mod test_outstation_event_responses;
mod test_outstation_frozen_counters;
mod test_outstation_state_machine;
mod test_outstation_unsolicited_responses;
mod test_static_data_map;
mod test_transport_layer;
mod test_typed_command_header;
mod test_update_builder;
mod test_write_conversions;
