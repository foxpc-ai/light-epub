use alloc::borrow::Cow;
use alloc::vec::Vec;

use nom::{
    IResult,
    bytes::complete::{tag, take},
    error::{Error, ErrorKind},
    number::complete::le_u16,
};
use quick_xml::{events::Event, reader::Reader};

use crate::{
    central_directory::find_entry, central_directory::find_eocd, central_directory::parse_eocd,
    central_directory::parse_local_file, errors::EpubError,
};

const ZIP_MAGIC: &[u8] = b"PK\x03\x04";
const MIMETYPE_NAME: &[u8] = b"mimetype";
const MIMETYPE_CONTENT: &[u8] = b"application/epub+zip";

pub struct ZipEntry {
    pub offset: u32,
    pub compressed_size: u32,
    pub compression_method: u16,
    pub uncompressed_size: u32,
}

pub struct ZipContext {
    pub cd_offset: u32,
    pub file_count: u16,
}

pub(crate) fn valid_epub(in_byte: &[u8]) -> Result<(), EpubError> {
    parse_epub_header(in_byte).map(|_| ()).map_err(|e| match e {
        nom::Err::Incomplete(_) => EpubError::BufferTooShort,
        nom::Err::Error(err) | nom::Err::Failure(err) => match err.code {
            nom::error::ErrorKind::Tag => EpubError::InvalidZipMagic,
            nom::error::ErrorKind::Verify => EpubError::MimetypeInvalid,
            _ => EpubError::Unknown,
        },
    })
}

/// Validates the ePub OCF header.
/// 1. Must start with Local File Header Magic.
/// 2. Compression method MUST be 0 (Stored).
/// 3. Filename MUST be "mimetype".
/// 4. Content MUST be "application/epub+zip".
pub(crate) fn parse_epub_header(input: &[u8]) -> IResult<&[u8], ()> {
    let (input, _) = tag(ZIP_MAGIC)(input)?;

    let (input, _) = take(4usize)(input)?;

    let (input, compression) = le_u16(input)?;
    if compression != 0 {
        return Err(nom::Err::Failure(Error::new(input, ErrorKind::Tag)));
    }

    let (input, _) = take(16usize)(input)?;

    let (input, name_len) = le_u16(input)?;
    let (input, extra_len) = le_u16(input)?;

    if name_len != 8 {
        return Err(nom::Err::Failure(Error::new(input, ErrorKind::Verify)));
    }

    let (input, _) = tag(MIMETYPE_NAME)(input)?;

    let (input, _) = take(extra_len)(input)?;

    let (input, _) = tag(MIMETYPE_CONTENT)(input)?;

    Ok((input, ()))
}

pub(crate) fn find_offset_and_size(
    in_byte: &[u8],
    target: &[u8],
    offset: Option<u32>,
    count: Option<u16>,
) -> Result<(ZipEntry, ZipContext), EpubError> {
    let (current_offset, current_count) = if let (Some(o), Some(c)) = (offset, count) {
        (o, c)
    } else {
        let eocd_index = find_eocd(in_byte).ok_or(EpubError::EocdNotFound)?;
        let (_, (found_count, found_offset)) =
            parse_eocd(&in_byte[eocd_index..]).map_err(|_| EpubError::InvalidZipMagic)?;
        (found_offset, found_count)
    };

    let cd_slice = in_byte
        .get(current_offset as usize..)
        .ok_or(EpubError::BufferTooShort)?;

    let entry = find_entry(cd_slice, current_count, target).ok_or(EpubError::FileNotFound)?;

    Ok((
        ZipEntry {
            offset: entry.offset,
            compressed_size: entry.compressed_size,
            compression_method: entry.compressed_method,
            uncompressed_size: entry.uncompressed_size,
        },
        ZipContext {
            cd_offset: current_offset,
            file_count: current_count,
        },
    ))
}

pub(crate) fn get_content<'a>(
    input: &'a [u8],
    target: &[u8],
    offset: Option<u32>,
    count: Option<u16>,
) -> Result<Cow<'a, [u8]>, EpubError> {
    let (zipentry, _) = find_offset_and_size(input, target, offset, count)?;

    let local_header_area = input
        .get(zipentry.offset as usize..)
        .ok_or(EpubError::BufferTooShort)?;

    let (data_start, _) =
        parse_local_file(local_header_area).map_err(|_| EpubError::InvalidZipMagic)?;

    let file_bytes = data_start
        .get(..zipentry.compressed_size as usize)
        .ok_or(EpubError::BufferTooShort)?;

    match zipentry.compression_method {
        0 => Ok(Cow::Borrowed(file_bytes)),
        8 => {
            let decompressed = miniz_oxide::inflate::decompress_to_vec_with_limit(
                file_bytes,
                zipentry.uncompressed_size as usize,
            )
            .map_err(|_| EpubError::DeflateError)?;

            Ok(Cow::Owned(decompressed))
        }
        _ => Err(EpubError::Unknown),
    }
}

pub(crate) fn get_content_buffer<'b>(
    input: &[u8],
    target: &[u8],
    offset: Option<u32>,
    count: Option<u16>,
    scratch_buf: &'b mut [u8],
) -> Result<&'b [u8], EpubError> {
    let (zipentry, _) = find_offset_and_size(input, target, offset, count)?;

    let local_header_area = input
        .get(zipentry.offset as usize..)
        .ok_or(EpubError::BufferTooShort)?;

    let (data_start, _) =
        parse_local_file(local_header_area).map_err(|_| EpubError::InvalidZipMagic)?;

    let file_bytes = data_start
        .get(..zipentry.compressed_size as usize)
        .ok_or(EpubError::BufferTooShort)?;

    match zipentry.compression_method {
        0 => {
            if scratch_buf.len() < zipentry.compressed_size as usize {
                return Err(EpubError::ScratchBufferTooSmall);
            }
            scratch_buf[..zipentry.compressed_size as usize].copy_from_slice(file_bytes);
            Ok(&scratch_buf[..zipentry.compressed_size as usize])
        }
        8 => {
            if scratch_buf.len() < zipentry.uncompressed_size as usize {
                return Err(EpubError::ScratchBufferTooSmall);
            }
            miniz_oxide::inflate::decompress_slice_iter_to_slice(
                scratch_buf,
                core::iter::once(file_bytes),
                false,
                false,
            )
            .map(|bytes_written| &scratch_buf[..bytes_written])
            .map_err(|_| EpubError::DeflateError)
        }
        _ => Err(EpubError::DeflateError),
    }
}

pub(crate) fn extract_opf_path<'a>(content: &'a [u8]) -> Option<Cow<'a, str>> {
    let mut reader = Reader::from_reader(content);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => return None,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"rootfile" {
                    let attr = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .find(|a| a.key.as_ref() == b"full-path")?;

                    let unescaped_cow = attr.decode_and_unescape_value(reader.decoder()).ok()?;

                    return match unescaped_cow {
                        Cow::Owned(s) => Some(Cow::Owned(s)),
                        Cow::Borrowed(_) => {
                            let raw_value = attr.value;
                            let decoded = reader.decoder().decode(&raw_value).ok()?;
                            Some(Cow::Owned(alloc::string::String::from(decoded)))
                        }
                    };
                }
            }
            _ => (),
        }
        buf.clear();
    }
}
