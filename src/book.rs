use crate::{
    errors::EpubError,
    nav,
    nav::NavItem,
    ocf::{extract_opf_path, find_offset_and_size, get_content, get_content_buffer, valid_epub},
    opf::{Package, parse_package, resolve_path},
};
use alloc::borrow::Cow;
use alloc::vec::Vec;

/// Represents a loaded EPUB book.
/// Holds the parsed structure and a reference to the raw zip buffer.
pub struct Book {
    /// The parsed metadata, manifest, and spine information.
    pub package: Package,
    /// The raw byte buffer of the EPUB file (usually the loaded .epub file).
    // buffer: &'a [u8],
    offset: u32,
    file_count: u16,
    // pub scratch_buffer: Option<&'b mut [u8]>,
}

impl Book {
    /// THE FAST PATH (Static Method)
    /// Use this to quickly extract Title/Author without parsing the whole book.
    pub fn get_metadata(data: &[u8]) -> Result<Package, EpubError> {
        valid_epub(data)?;

        let (_, context) = find_offset_and_size(data, b"META-INF/container.xml", None, None)?;

        let container_content = get_content(
            data,
            b"META-INF/container.xml",
            Some(context.cd_offset),
            Some(context.file_count),
        )?;

        let opf_path = extract_opf_path(&container_content).ok_or(EpubError::OpfNotFound)?;

        let opf_data = get_content(
            data,
            opf_path.as_bytes(),
            Some(context.cd_offset),
            Some(context.file_count),
        )?;

        parse_package(&opf_data, &opf_path, true).ok_or(EpubError::MalformedXml)
    }

    pub fn new(data: &[u8]) -> Result<Self, EpubError> {
        valid_epub(data)?;

        let (_, context) = find_offset_and_size(data, b"META-INF/container.xml", None, None)?;

        let container_content = get_content(
            data,
            b"META-INF/container.xml",
            Some(context.cd_offset),
            Some(context.file_count),
        )?;

        let opf_path = extract_opf_path(&container_content).ok_or(EpubError::OpfNotFound)?;

        let opf_data = get_content(
            data,
            opf_path.as_bytes(),
            Some(context.cd_offset),
            Some(context.file_count),
        )?;

        let package = parse_package(&opf_data, &opf_path, false).ok_or(EpubError::MalformedXml)?;

        Ok(Self {
            package,
            // buffer: data,
            offset: context.cd_offset,
            file_count: context.file_count,
            // scratch_buffer: scratch_buf,
        })
    }

    /// Returns the total number of chapters defined in the reading order (spine).
    pub fn total_chapters(&self) -> usize {
        self.package
            .spine
            .as_ref()
            .map(|s| s.items.len())
            .unwrap_or(0)
    }

    /// Returns the title of the book.
    pub fn title(&self) -> &str {
        &self.package.title
    }

    /// Returns the author/creator of the book.
    pub fn author(&self) -> &str {
        &self.package.author
    }

    /// Retrieves the content of a specific chapter by its index.
    /// Returns a Cow<'a, [u8]> which is a zero-copy reference to the original buffer.
    pub fn get_chapter<'a, 'b>(
        &self,
        data: &'a [u8],
        index: usize,
        scratch_buf: Option<&'b mut [u8]>,
    ) -> Result<Cow<'b, [u8]>, EpubError>
    where
        'a: 'b,
    {
        let spine = self.package.spine.as_ref().ok_or(EpubError::OpfNotFound)?;
        let item = spine.items.get(index).ok_or(EpubError::FileNotFound)?;

        if let Some(sb) = scratch_buf {
            let result = get_content_buffer(
                data,
                item.href.as_bytes(),
                Some(self.offset),
                Some(self.file_count),
                sb,
            )?;
            Ok(Cow::Borrowed(result))
        } else {
            get_content(
                data,
                item.href.as_bytes(),
                Some(self.offset),
                Some(self.file_count),
            )
        }
    }

    /// Returns the unique ID of the chapter.
    pub fn get_chapter_id(&self, index: usize) -> Option<&str> {
        let spine = self.package.spine.as_ref()?;
        let item = spine.items.get(index)?;
        Some(&item.idref)
    }

    pub fn get_resource<'a>(
        &self,
        data: &'a [u8],
        chapter_idx: usize,
        relative_path: &str,
    ) -> Result<Cow<'a, [u8]>, EpubError> {
        let spine = self.package.spine.as_ref().ok_or(EpubError::OpfNotFound)?;
        let chapter_item = spine
            .items
            .get(chapter_idx)
            .ok_or(EpubError::FileNotFound)?;

        let mut path_buf = [0u8; 256];
        let resolved = resolve_path(&chapter_item.href, relative_path, &mut path_buf)
            .ok_or(EpubError::FileNotFound)?;

        get_content(
            data,
            resolved.as_bytes(),
            Some(self.offset),
            Some(self.file_count),
        )
    }

    /// Returns the Table of Contents (Navigation) for the book.
    /// Supports both EPUB 2 (NCX) and EPUB 3 (NAV) formats.
    pub fn get_toc(&self, data: &[u8]) -> Result<Vec<NavItem>, EpubError> {
        let toc_path = self.package.toc.as_ref().ok_or(EpubError::OpfNotFound)?;

        let toc_content = get_content(
            data,
            toc_path.as_bytes(),
            Some(self.offset),
            Some(self.file_count),
        )?;

        if toc_path.ends_with(".ncx") {
            Ok(nav::parse_toc_ncx(&toc_content))
        } else {
            Ok(nav::parse_nav_xhtml(&toc_content))
        }
    }

    /// Helper to get any resource by its absolute path within the zip.
    pub fn get_resource_by_path<'a>(
        &self,
        data: &'a [u8],
        path: &str,
    ) -> Result<Cow<'a, [u8]>, EpubError> {
        get_content(
            data,
            path.as_bytes(),
            Some(self.offset),
            Some(self.file_count),
        )
    }

    // Helper to get data from the book without constructing the struct.
    pub fn get_raw_content<'a>(
        buf: &'a [u8],
        target: &[u8],
        offset: Option<u32>,
        count: Option<u16>,
    ) -> Result<Cow<'a, [u8]>, EpubError> {
        get_content(buf, target, offset, count)
    }
}
