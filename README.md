This crate can parse GPT and basic MBR partition tables.

[![Github CI](https://github.com/FauxFaux/bootsector/actions/workflows/rust.yml/badge.svg)](https://github.com/FauxFaux/bootsector/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/bootsector.svg)](https://crates.io/crates/bootsector)


### Documentation and Examples

https://docs.rs/bootsector

### Limitations

 * MBR extended partitions are not read (although they are returned, so you could read
   them yourself). This should be implemented.
 * GPT backup tables are not validated, which is "kinda" required by the spec. This
   could be implemented, but isn't super important, unless you're doing data recovery.
 * Sector sizes apart from 512 bytes are not well tested. These devices don't seem to
   exist as of 2017.

### MSRV

Rust 1.34 (`TryFrom`) is supported, and checked by CI.
Updating this is a semver bump.
