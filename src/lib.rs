//! paper2horizon: reads a Paper or Bukkit plugin's `.jar` and writes the same plugin's shape as a
//! horizon-plugin-api crate, with what's left to port marked in it and in `PORTING.md`.

pub mod analysis;
pub mod classfile;
pub mod generate;
pub mod mapping;
