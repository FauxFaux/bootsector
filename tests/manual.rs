extern crate bootsector;

use std::io;

use bootsector::list_partitions;
use bootsector::Attributes;
use bootsector::Options;


#[test]
fn four_tee_gpt() {
    let parts = list_partitions(
        cursor(include_bytes!("test-data/4t-gpt.img")),
        &Options::default(),
    ).expect("success");

    assert_eq!(2, parts.len());

    assert_eq!(0, parts[0].id);
    assert_eq!(1024 * 1024, parts[0].first_byte);
    assert_eq!(3_000_999_346_176, parts[0].len);

    assert_eq!(1, parts[1].id);
    assert_eq!(3_001_000_394_752, parts[1].first_byte);
    assert_eq!(999_786_618_368, parts[1].len);

    // TODO: uuids
}


#[test]
fn fdisk_1m_part() {
    let parts = list_partitions(
        cursor(include_bytes!("test-data/fdisk-1m-part.img")),
        &Options::default(),
    ).expect("success");

    assert_eq!(1, parts.len());

    assert_eq!(0, parts[0].id);
    assert_eq!(34 * 512, parts[0].first_byte);
    assert_eq!(1024 * 1024, parts[0].len);

    // TODO: uuids
}


#[test]
fn fdisk_empty_gpt() {
    let parts = list_partitions(
        cursor(include_bytes!("test-data/fdisk-empty-gpt.img")),
        &Options::default(),
    ).expect("success");

    assert_eq!(0, parts.len());
}

#[test]
fn fdisk_empty_mbr() {
    let parts = list_partitions(
        cursor(include_bytes!("test-data/fdisk-empty-mbr.img")),
        &Options::default(),
    ).expect("success");

    assert_eq!(0, parts.len());
}

#[test]
fn ubu_raspi() {
    let parts = list_partitions(
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
        _ => panic!(),
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
        _ => panic!(),
    }

    assert_eq!(138412032, parts[1].first_byte);
    assert_eq!(3999268864, parts[1].len);
}

#[test]
fn require_mbr() {
    let mut options = Options::default();
    options.gpt = bootsector::ReadGPT::Never;

    let parts = list_partitions(cursor(include_bytes!("test-data/4t-gpt.img")), &options).unwrap();

    assert_eq!(1, parts.len());
    match parts[0].attributes {
        Attributes::MBR {
            type_code,
            bootable: _,
        } => assert_eq!(0xEE, type_code),
        _ => panic!("not a protective partition on a gpt volume"),
    }
}

#[test]
fn require_gpt() {
    let mut options = Options::default();
    options.mbr = bootsector::ReadMBR::Never;

    assert_eq!(
        io::ErrorKind::NotFound,
        list_partitions(
            cursor(include_bytes!("test-data/mbr-ubuntu-raspi3-16.04.img")),
            &options,
        ).unwrap_err()
            .kind()
    );
}

fn cursor(bytes: &[u8]) -> io::Cursor<&[u8]> {
    io::Cursor::new(bytes)
}
