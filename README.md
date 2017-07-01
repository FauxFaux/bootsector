This crate can parse GPT and basic MBR partition tables.

[![Build status](https://api.travis-ci.org/FauxFaux/bootsector.png)](https://travis-ci.org/FauxFaux/bootsector)
[![](http://meritbadge.herokuapp.com/bootsector)](https://crates.io/crates/bootsector)


### Documentation and Examples

https://docs.rs/bootsector

### Limitations

 * MBR extended partitions are not read (although they are returned, so you could read
   them yourself). This should be implemented.
 * GPT backup tables are not validated, which is "kinda" required by the spec. This
   could be implemented, but isn't super important, unless you're doing data recovery.
 * Sector sizes apart from 512 bytes are not well tested. These devices don't seem to
   exist as of 2017.
