extern crate self as bytepack;

mod pack;
mod pack_pointer;
mod packer;
mod packer_format;
mod unpacker;

pub use pack::*;
pub use pack_pointer::*;
pub use packer::*;
pub use packer_format::*;
pub use unpacker::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_format() {
        let format = PackerFormat::new(
            [
                PackerField::new("name", String::PACK_BYTES),
                PackerField::new("age", u8::PACK_BYTES),
                PackerField::new("adress", String::PACK_BYTES),
                PackerField::new("cat_names", Vec::<String>::PACK_BYTES),
            ]
            .into_iter(),
        );

        let mut bytes = BytePacker::new(format.fixed_byte_count());
        let mut packer = bytes.fields(&format, 0);

        packer.pack("name", &"Peter".to_owned());
        packer.pack("age", &32_u8);
        packer.pack("adress", &"Peterweg 26".to_owned());
        packer.pack(
            "cat_names",
            &vec![
                "Pusheen".to_owned(),
                "Plusheen".to_owned(),
                "Gloobert".to_owned(),
            ],
        );

        let bytes = bytes.finish();

        let unpacker = ByteUnpacker::new(&bytes);
        let unpacker = unpacker.fields(&format, 0);

        let name = unpacker.unpack::<String>("name").unwrap();
        let age = unpacker.unpack::<u8>("age").unwrap();
        let adress = unpacker.unpack::<String>("adress").unwrap();
        let cat_names = unpacker.unpack::<Vec<String>>("cat_names").unwrap();

        assert_eq!(name, "Peter");
        assert_eq!(age, 32_u8);
        assert_eq!(adress, "Peterweg 26");
        assert_eq!(cat_names.len(), 3);
        assert_eq!(cat_names[0], "Pusheen");
        assert_eq!(cat_names[1], "Plusheen");
        assert_eq!(cat_names[2], "Gloobert");
    }

    #[test]
    fn pack_unpack_no_format() {
        let value = vec!["hey".to_owned(), "123".to_owned(), "foobar".to_owned()];

        let mut packer = BytePacker::new(Vec::<String>::PACK_BYTES);

        value.pack(0, &mut packer);

        let bytes = packer.finish();

        let unpacker = ByteUnpacker::new(&bytes);

        let result = Vec::<String>::unpack(0, &unpacker);

        assert_eq!(Some(value), result);
    }
}
