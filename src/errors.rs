#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EpubError {
    BufferTooShort,
    InvalidZipMagic,
    EocdNotFound,
    FileNotFound,
    DeflateError,
    MimetypeMissing,
    MimetypeInvalid,
    MimetypeCompressed,
    ContainerNotFound,
    OpfNotFound,
    MalformedXml,
    ScratchBufferTooSmall,
    Unknown,
}

impl core::fmt::Display for EpubError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooShort => write!(f, "Provided buffer is too small"),
            Self::InvalidZipMagic => write!(f, "Not a valid ZIP (Local Header magic mismatch)"),
            Self::EocdNotFound => write!(f, "Could not find End of Central Directory record"),
            Self::FileNotFound => write!(f, "Requested file not found in ZIP"),
            Self::DeflateError => write!(f, "Failed to decompress DEFLATE stream"),
            Self::MimetypeMissing => write!(f, "mimetype file must be the first entry in ZIP"),
            Self::MimetypeInvalid => write!(f, "mimetype content must be 'application/epub+zip'"),
            Self::MimetypeCompressed => write!(f, "mimetype file must not be compressed"),
            Self::ContainerNotFound => write!(f, "META-INF/container.xml not found"),
            Self::OpfNotFound => write!(f, "Could not locate .opf file path in container"),
            Self::MalformedXml => write!(f, "Failed to parse XML content"),
            Self::ScratchBufferTooSmall => {
                write!(f, "Scratch buffer is smaller than uncompressed size")
            }
            Self::Unknown => write!(f, "An unknown parsing error occurred"),
        }
    }
}
