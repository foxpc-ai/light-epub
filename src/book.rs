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
/// Holds the parsed structure and a reference to the raw zip buffer's layout.
pub struct Book {
    /// The parsed metadata, manifest, and spine information.
    /// Access .title, .author, or .spine directly from here.
    pub package: Package,
    offset: u32,
    file_count: u16,
}

impl Book {
    /// THE FAST PATH (Static Method)
    /// Quickly extract metadata without keeping the book structure in memory.
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

    /// Creates a new Book instance by parsing the container and OPF package.
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
            offset: context.cd_offset,
            file_count: context.file_count,
        })
    }

    /// Internal helper to centralize ZIP extraction using the cached offset/count.
    fn fetch<'a, 'b>(
        &self,
        data: &'a [u8],
        path: &[u8],
        scratch: Option<&'b mut [u8]>,
    ) -> Result<Cow<'b, [u8]>, EpubError>
    where
        'a: 'b,
    {
        if let Some(sb) = scratch {
            let result =
                get_content_buffer(data, path, Some(self.offset), Some(self.file_count), sb)?;
            Ok(Cow::Borrowed(result))
        } else {
            get_content(data, path, Some(self.offset), Some(self.file_count))
        }
    }

    /// Retrieves the content of a specific chapter by its index in the spine.
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
        self.fetch(data, item.href.as_bytes(), scratch_buf)
    }

    /// Resolves and retrieves a resource (like an image) relative to a specific chapter.
    pub fn get_resource<'a, 'b>(
        &self,
        data: &'a [u8],
        chapter_idx: usize,
        relative_path: &str,
        scratch_buf: Option<&'b mut [u8]>,
    ) -> Result<Cow<'b, [u8]>, EpubError>
    where
        'a: 'b,
    {
        let spine = self.package.spine.as_ref().ok_or(EpubError::OpfNotFound)?;
        let chapter_item = spine
            .items
            .get(chapter_idx)
            .ok_or(EpubError::FileNotFound)?;

        let mut path_buf = [0u8; 256];
        let resolved = resolve_path(&chapter_item.href, relative_path, &mut path_buf)
            .ok_or(EpubError::FileNotFound)?;

        self.fetch(data, resolved.as_bytes(), scratch_buf)
    }

    /// Returns the Table of Contents (Navigation) for the book.
    pub fn get_toc(&self, data: &[u8]) -> Result<Vec<NavItem>, EpubError> {
        let toc_path = self.package.toc.as_ref().ok_or(EpubError::OpfNotFound)?;
        let toc_content = self.fetch(data, toc_path.as_bytes(), None)?;

        let spine_items = self
            .package
            .spine
            .as_ref()
            .map(|s| s.items.as_slice())
            .unwrap_or(&[]);

        if toc_path.ends_with(".ncx") {
            Ok(nav::parse_toc_ncx(&toc_content, spine_items))
        } else {
            Ok(nav::parse_nav_xhtml(&toc_content, spine_items))
        }
    }

    /// Helper to get any resource by its absolute path within the book.
    pub fn get_resource_by_path<'a, 'b>(
        &self,
        data: &'a [u8],
        path: &str,
        scratch_buf: Option<&'b mut [u8]>,
    ) -> Result<Cow<'b, [u8]>, EpubError>
    where
        'a: 'b,
    {
        self.fetch(data, path.as_bytes(), scratch_buf)
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
