pub mod carrier_toml;
pub mod cran;          
pub mod formats;
pub mod manifest;
pub mod ops;
pub mod paths;
pub mod version;    

pub use carrier_toml::CarrierToml;
pub use manifest::Manifest;
pub use paths::{resolve_install_dir, resolve_r_lib_dir};
