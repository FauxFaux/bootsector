extern crate bootsector;

use std::io;

use bootsector::open;
use bootsector::Attributes;
use bootsector::Options;

#[test]
fn ubu_raspi() {
    let parts = open(
        cursor(include_bytes!("test-data/mbr-ubuntu-raspi3-16.04.img")),
        &Options::default(),
    ).expect("success");

    assert_eq!(2, parts.len());

    assert_eq!(0, parts[0].id);
    match parts[0].attributes {
        Attributes::MBR {
            bootable,
            type_code,
        } => {
            assert_eq!(true, bootable);
            assert_eq!(12, type_code);
        }
        _ => panic!()
    }
    assert_eq!(4194304, parts[0].first_byte);
    assert_eq!(138412032, parts[0].len);

    assert_eq!(1, parts[1].id);
    match parts[1].attributes {
        Attributes::MBR {
            bootable,
            type_code,
        } => {
            assert_eq!(false, bootable);
            assert_eq!(131, type_code);
        }
        _ => panic!()
    }

    assert_eq!(138412032, parts[1].first_byte);
    assert_eq!(3999268864, parts[1].len);
}

fn cursor(bytes: &[u8]) -> io::Cursor<&[u8]> {
    io::Cursor::new(bytes)
}
