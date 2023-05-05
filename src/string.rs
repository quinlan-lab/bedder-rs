#[cfg(feature = "smol_str")]
pub use smol_str::SmolStr as String;

#[cfg(feature = "smartstring")]
pub use smartstring::alias::String;

#[cfg(feature = "compact_str")]
pub use compact_str::CompactString as String;

#[cfg(feature = "kstring")]
pub use kstring::KString as String;

#[cfg(all(
    not(feature = "smartstring"),
    not(feature = "smol_str"),
    not(feature = "compact_str"),
    not(feature = "kstring"),
))]
pub use std::string::String;
