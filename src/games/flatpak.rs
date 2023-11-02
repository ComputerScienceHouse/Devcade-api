use glib::variant::{DictEntry, FixedSizeVariantArray, FromVariant, Variant};
use glib::{StaticVariantType, VariantDict, VariantTy};
use lazy_static::lazy_static;
use std::any::TypeId;
use std::error::Error;
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;

// /// The type of a commit object: `(a{sv}aya(say)sstayay)`
// /// Ripped from: https://docs.rs/ostree/0.19.1/src/ostree/core.rs.html#5-15
// type CommitVariantType = (
//     VariantDict,
//     Vec<u8>,
//     Vec<(String, Vec<u8>)>,
//     String,
//     String,
//     u64,
//     Vec<u8>,
//     Vec<u8>,
// );

// #[derive(glib::Variant)]
// struct OstreeStaticDeltaMetaEntryFormat {
//     version: u32,                                         // u
//     checksum: FixedSizeVariantArray<Vec<u8>, u8>,         // ay
//     size: u64,                                            // t
//     uncompressed_size: u64,                               // t
//     checksum_objects: FixedSizeVariantArray<Vec<u8>, u8>, // ay
// }

// #[derive(glib::Variant)]
// struct OstreeStaticDeltaFallbackFormat {
//     objtype: u8,                                  // y
//     checksum: FixedSizeVariantArray<Vec<u8>, u8>, // ay
//     compressed_size: u64,                         // t
//     uncompressed_size: u64,                       // t
// }

// #[derive(glib::Variant)]
// pub struct FlatpakFile {
//     metadata: Vec<DictEntry<String, Variant>>,
//     _unknown1: u64,
//     _unknown2: FixedSizeVariantArray<Vec<u8>, u8>,
//     checksum: FixedSizeVariantArray<Vec<u8>, u8>,
//     ostree_commit_dummy: (VariantDict,),
//     // ostree_commit: CommitVariantType,
//     // _unknown3: FixedSizeVariantArray<Vec<u8>, u8>,
//     // ostree_static_delta_meta_entries: Vec<OstreeStaticDeltaMetaEntryFormat>,
//     // ostree_static_delta_fallbacks: Vec<OstreeStaticDeltaFallbackFormat>,
// }

// #[test]
// fn test_flatpak_schema() {
//     assert_eq!(
//         FlatpakFile::static_variant_type().as_str(),
//         "(a{sv}tayay(a{sv}aya(say)sstayay)aya(uayttay)a(yaytt))",
//     );
// }

pub struct FlatpakFile(Variant);

lazy_static! {
    static ref FLATPAK_FILE_VARIANT: &'static VariantTy =
        VariantTy::new("(a{sv}tayay(a{sv}aya(say)sstayay)aya(uayttay)a(yaytt))").unwrap();
}

#[test]
/// Make sure the [`VariantTy`] for the `FLATPAK_FILE_VARIANT` is valid
fn check_flatpak_variant_type() {
    let _ = *FLATPAK_FILE_VARIANT;
}

#[derive(Debug, Clone)]
pub enum FlatpakMetadataError<T: FromVariant + Debug + 'static> {
    MissingKey(String),
    IncorrectFormat(String, PhantomData<T>),
}

impl<T: FromVariant + Debug + 'static> Error for FlatpakMetadataError<T> {}

impl<T: FromVariant + Debug + 'static> Display for FlatpakMetadataError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FlatpakMetadataError::*;
        match self {
            MissingKey(key_name) => write!(f, "Flatpak metadata incorrect: missing key {key_name}"),
            IncorrectFormat(key_name, _type) => write!(
                f,
                "Flatpak metadata incorrect: couldn't read key {key_name} of type {:?}",
                TypeId::of::<T>()
            ),
        }
    }
}

impl FlatpakFile {
    pub fn get_metadata_key<T: FromVariant + Debug>(
        &self,
        key: &str,
    ) -> Result<T, FlatpakMetadataError<T>> {
        let dict_array = self.0.child_value(0);
        for index in 0..dict_array.n_children() {
            let dict_entry = dict_array.child_value(index);
            if let Some(candidate_key) = String::from_variant(&dict_entry.child_value(0)) {
                if candidate_key == key {
                    let value = dict_entry.child_value(1);
                    let value = match value.as_variant() {
                        Some(value) => value,
                        None => value,
                    };
                    return T::from_variant(&value).ok_or_else(|| {
                        FlatpakMetadataError::IncorrectFormat(key.to_string(), PhantomData {})
                    });
                };
            }
        }
        Err(FlatpakMetadataError::MissingKey(key.to_string()))
    }
    pub fn load<T: AsRef<[u8]>>(bytes: T) -> Result<Self, FlatpakDecodingError> {
        let variant = Variant::from_data_with_type(bytes, &FLATPAK_FILE_VARIANT);
        let metadata = variant.child_value(0);
        if !metadata.is_container() {
            return Err(FlatpakDecodingError::MetadataNotContainer);
        }
        for index in 0..metadata.n_children() {
            let child = metadata.child_value(index);
            if !child.is_container() || child.n_children() != 2 {
                return Err(FlatpakDecodingError::MetadataChildNotContainer);
            }
        }
        let checksum = variant.child_value(3);
        if !checksum.is_container() || checksum.n_children() != 32 {
            return Err(FlatpakDecodingError::BadChecksumLength);
        }
        Ok(FlatpakFile(variant))
    }

    pub fn get_hash(&self) -> String {
        hex::encode(<Vec<u8> as FromVariant>::from_variant(&self.0.child_value(3)).unwrap())
    }
}

#[derive(Debug, Clone)]
pub enum FlatpakDecodingError {
    IncorrectFormat,
    MetadataNotContainer,
    MetadataChildNotContainer,
    BadChecksumLength,
}

impl Error for FlatpakDecodingError {}
impl Display for FlatpakDecodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncorrectFormat => write!(f, "Incorrect flatpak bundle format"),
            Self::BadChecksumLength => write!(f, "Flatpak bundle checksum is the incorrect length"),
            Self::MetadataNotContainer => {
                write!(f, "Flatpak bundle metadata field isn't a container")
            }
            Self::MetadataChildNotContainer => {
                write!(f, "Flatpak bundle metadata child isn't a container")
            }
        }
    }
}
