//! DNP3 enumerations (function codes, qualifiers, group/variations, ...).
//!
//! Values are defined by the DNP3 (IEEE 1815) specification and correspond to
//! the generated enums in the original C++ library.

mod app;
mod channel;
mod flags;
mod link;
mod measurement;
mod task;

pub use app::*;
pub use channel::*;
pub use flags::*;
pub use link::*;
pub use measurement::*;
pub use task::*;

/// Declare a `#[repr(u8)]` enum with `to_u8` / `from_u8` conversions and a
/// `Unknown`-style fallback variant.
macro_rules! dnp_enum {
    (
        $(#[$meta:meta])*
        $name:ident, $unknown:ident;
        $( $(#[$vmeta:meta])* $variant:ident = $value:expr ),* $(,)?
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum $name {
            $( $(#[$vmeta])* $variant = $value, )*
            /// Value on the wire did not match any known variant.
            $unknown = 0xFF,
        }

        impl $name {
            /// The raw protocol value.
            pub const fn to_u8(self) -> u8 {
                self as u8
            }

            /// Convert a raw protocol value; unknown values map to
            /// [`Self::$unknown`].
            pub const fn from_u8(value: u8) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    _ => Self::$unknown,
                }
            }
        }
    };
}

pub(crate) use dnp_enum;
