//! 1:1 port of `TestLinkAddresses.cpp` (2 test cases).

use opendnp3::link::Addresses;

#[test]
fn link_addresses_equality_comparison() {
    let lr1 = Addresses::new(1, 2);
    let lr2 = Addresses::new(1, 3);
    let lr3 = Addresses::new(1, 3);

    assert_ne!(lr1, lr2);
    assert_eq!(lr3, lr2);
}

#[test]
fn link_addresses_is_broadcast() {
    assert!(Addresses::new(1, 0xFFFD).is_broadcast());
    assert!(Addresses::new(1, 0xFFFE).is_broadcast());
    assert!(Addresses::new(1, 0xFFFF).is_broadcast());
    assert!(!Addresses::new(1, 1024).is_broadcast());
}
