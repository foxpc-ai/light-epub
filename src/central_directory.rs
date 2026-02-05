use nom::{
    IResult,
    bytes::complete::{tag, take},
    number::{complete::le_u16, complete::le_u32},
};

const CD_MAGIC: &[u8] = b"PK\x01\x02";
const LOCAL_MAGIC: &[u8] = b"PK\x03\x04";
const EOCD_MAGIC: &[u8] = b"PK\x05\x06";

pub struct CentralDirectoryEntry<'a> {
    pub file_name: &'a [u8],
    pub offset: u32,
    pub compressed_size: u32,
    pub compressed_method: u16,
    pub uncompressed_size: u32,
}

pub(crate) fn parse_cd_header(input: &[u8]) -> IResult<&[u8], CentralDirectoryEntry<'_>> {
    let (input, _) = tag(CD_MAGIC)(input)?;
    let (input, _) = take(6usize)(input)?;
    let (input, method) = le_u16(input)?;

    let (input, _) = take(8usize)(input)?;
    let (input, compressed) = le_u32(input)?;
    let (input, uncompressed) = le_u32(input)?;

    let (input, name_len) = le_u16(input)?;
    let (input, extra_len) = le_u16(input)?;
    let (input, comment_len) = le_u16(input)?;

    let (input, _) = take(8usize)(input)?; // Skip disk start, internal/external attrs
    let (input, header_offset) = le_u32(input)?;

    let (input, file_name) = take(name_len)(input)?;

    let (input, _) = take(extra_len + comment_len)(input)?;

    Ok((
        input,
        CentralDirectoryEntry {
            file_name,
            offset: header_offset,
            compressed_size: compressed,
            compressed_method: method,
            uncompressed_size: uncompressed,
        },
    ))
}

pub(crate) fn find_entry<'a>(
    mut input: &'a [u8],
    count: u16,
    target: &[u8],
) -> Option<CentralDirectoryEntry<'a>> {
    for _ in 0..count {
        match parse_cd_header(input) {
            Ok((next_input, entry)) => {
                if entry.file_name == target {
                    return Some(entry);
                }
                input = next_input;
            }
            Err(_) => {
                break;
            }
        }
    }
    None
}

pub(crate) fn find_eocd(data: &[u8]) -> Option<usize> {
    let len = data.len();
    if len < 22 {
        return None;
    }

    let scan_limit = len.saturating_sub(65535 + 22);

    for i in (scan_limit..=len - 22).rev() {
        if &data[i..i + 4] == EOCD_MAGIC {
            let comment_len = u16::from_le_bytes([data[i + 20], data[i + 21]]) as usize;

            if i + 22 + comment_len == len {
                return Some(i);
            }
        }
    }
    None
}

pub(crate) fn parse_eocd(input: &[u8]) -> IResult<&[u8], (u16, u32)> {
    let (input, _) = tag(EOCD_MAGIC)(input)?;
    let (input, _) = take(6usize)(input)?;
    let (input, file_count) = le_u16(input)?;
    let (input, _) = take(4usize)(input)?;
    let (input, cd_start) = le_u32(input)?;

    Ok((input, (file_count, cd_start)))
}

pub fn parse_local_file(input: &[u8]) -> IResult<&[u8], ()> {
    let (input, _) = tag(LOCAL_MAGIC)(input)?;
    let (input, _) = take(22usize)(input)?;
    let (input, filename_length) = le_u16(input)?;
    let (input, ext) = le_u16(input)?;
    let (input, _) = take((filename_length + ext) as usize)(input)?;
    Ok((input, ()))
}
