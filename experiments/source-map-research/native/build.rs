//! Build script for the source-map research addon.
//!
//! Only `napi_build::setup()` runs here: it configures the linker for the Node
//! addon this crate produces. The crate is a standalone experiment, not a
//! workspace member, so this script is the whole build configuration.
/// Configures the N-API build environment for the addon.

fn main() {
    napi_build::setup();
}
