use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use quick_xml::{Reader, events::Event};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Package {
    pub title: String,
    pub author: String,
    pub spine: Option<Spine>,
    pub toc: Option<String>,
    pub cover: Option<String>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Spine {
    pub items: Vec<SpineItem>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SpineItem {
    pub idref: String,
    pub href: String,
}

#[derive(Default)]
struct RawData {
    title: String,
    author: String,
    manifest: Vec<(String, String)>,
    spine_idrefs: Vec<String>,
    toc_id: Option<String>,
    cover_id: Option<String>,
}

pub(crate) fn parse_package(
    opf_content: &[u8],
    base_path: &str,
    metadata_only: bool,
) -> Option<Package> {
    let mut reader = Reader::from_reader(opf_content);

    let mut buf = Vec::new();

    #[derive(PartialEq)]
    enum State {
        None,
        Metadata,
        Manifest,
        Spine,
    }
    #[derive(PartialEq)]
    enum Field {
        None,
        Title,
        Creator,
    }

    let mut state = State::None;
    let mut field = Field::None;
    let mut raw = RawData::default();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Err(_) => return None,

            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.local_name();
                let name_ref = name.as_ref();

                match (name_ref, &state) {
                    (b"metadata", _) => state = State::Metadata,
                    (b"manifest", _) => state = State::Manifest,
                    (b"spine", _) => state = State::Spine,

                    (b"item", State::Manifest) => {
                        let mut id = None;
                        let mut href = None;
                        let mut is_nav = false;
                        let mut is_cover = false;
                        let mut is_ncx = false;

                        for attr in e.html_attributes().filter_map(|a| a.ok()) {
                            match attr.key.as_ref() {
                                b"id" => {
                                    id = Some(
                                        attr.decode_and_unescape_value(reader.decoder())
                                            .ok()?
                                            .into_owned(),
                                    )
                                }
                                b"href" => {
                                    href = Some(
                                        attr.decode_and_unescape_value(reader.decoder())
                                            .ok()?
                                            .into_owned(),
                                    )
                                }
                                b"properties" => {
                                    let val = attr.value;
                                    if val.windows(3).any(|w| w == b"nav") {
                                        is_nav = true;
                                    }
                                    if val.windows(11).any(|w| w == b"cover-image") {
                                        is_cover = true;
                                    }
                                }
                                b"media-type" => {
                                    if attr.value.as_ref() == b"application/x-dtbncx+xml" {
                                        is_ncx = true;
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let (Some(i), Some(h)) = (id, href) {
                            if is_nav {
                                raw.toc_id = Some(i.clone());
                            } else if is_ncx && raw.toc_id.is_none() {
                                raw.toc_id = Some(i.clone());
                            }
                            if is_cover {
                                raw.cover_id = Some(i.clone());
                            }
                            raw.manifest.push((i, h));
                        }
                    }

                    (b"itemref", State::Spine) if !metadata_only => {
                        for attr in e.html_attributes().filter_map(|a| a.ok()) {
                            if attr.key.as_ref() == b"idref" {
                                raw.spine_idrefs.push(
                                    attr.decode_and_unescape_value(reader.decoder())
                                        .ok()?
                                        .into_owned(),
                                );
                            }
                        }
                    }

                    (b"title", State::Metadata) => field = Field::Title,
                    (b"creator", State::Metadata) => field = Field::Creator,

                    (b"meta", State::Metadata) => {
                        let mut is_cover_meta = false;
                        let mut content = None;

                        for attr_result in e.html_attributes() {
                            let attr = match attr_result {
                                Ok(a) => a,
                                Err(_) => continue,
                            };

                            match attr.key.as_ref() {
                                b"name" => {
                                    if attr.value.as_ref() == b"cover" {
                                        is_cover_meta = true;
                                    }
                                }
                                b"content" => {
                                    content = attr
                                        .decode_and_unescape_value(reader.decoder())
                                        .ok()
                                        .map(|v| v.into_owned());
                                }
                                _ => {}
                            }
                        }

                        if is_cover_meta {
                            raw.cover_id = content;
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::Text(e)) if state == State::Metadata => {
                if field != Field::None
                    && let Ok(decoded) = reader.decoder().decode(e.as_ref())
                {
                    let text = decoded.trim().to_owned();
                    if field == Field::Title {
                        raw.title = text;
                    } else {
                        raw.author = text;
                    }
                }
            }

            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"metadata" | b"manifest" | b"spine" => state = State::None,
                b"title" | b"creator" => field = Field::None,
                _ => {}
            },
            _ => (),
        }
        buf.clear();
    }

    if raw.title.is_empty() && raw.spine_idrefs.is_empty() {
        return None;
    }

    let mut path_buf = [0u8; 512];

    let mut manifest = core::mem::take(&mut raw.manifest);

    let mut extract_item = |id: &str| {
        manifest
            .iter()
            .position(|(m_id, _)| m_id == id)
            .map(|idx| manifest.swap_remove(idx))
    };

    let toc = raw
        .toc_id
        .and_then(|id| extract_item(&id))
        .and_then(|(_, h)| resolve_path(base_path, &h, &mut path_buf))
        .map(String::from);

    let cover = raw
        .cover_id
        .and_then(|id| extract_item(&id))
        .and_then(|(_, h)| resolve_path(base_path, &h, &mut path_buf))
        .map(String::from);

    let spine = if metadata_only {
        None
    } else {
        let items: Vec<SpineItem> = raw
            .spine_idrefs
            .into_iter()
            .filter_map(|id| {
                let (old_id, old_href) = extract_item(&id)?;
                let res = resolve_path(base_path, &old_href, &mut path_buf)?;

                Some(SpineItem {
                    idref: old_id,
                    href: String::from(res),
                })
            })
            .collect();
        Some(Spine { items })
    };

    Some(Package {
        title: raw.title,
        author: raw.author,
        spine,
        toc,
        cover,
    })
}

pub(crate) fn resolve_path<'a>(
    base: &str,
    relative: &str,
    output: &'a mut [u8],
) -> Option<&'a str> {
    let base_dir = if let Some(idx) = base.rfind('/') {
        &base[..idx]
    } else {
        ""
    };
    let mut cursor = 0;

    let mut push = |seg: &str| {
        if seg.is_empty() || seg == "." {
            return true;
        }
        if seg == ".." {
            let s = core::str::from_utf8(&output[..cursor]).unwrap_or("");
            cursor = s.rfind('/').unwrap_or(0);
            return true;
        }
        if cursor > 0 && output[cursor - 1] != b'/' {
            if cursor >= output.len() {
                return false;
            }
            output[cursor] = b'/';
            cursor += 1;
        }
        let b = seg.as_bytes();
        if cursor + b.len() > output.len() {
            return false;
        }
        output[cursor..cursor + b.len()].copy_from_slice(b);
        cursor += b.len();
        true
    };

    if !base_dir.is_empty() {
        for s in base_dir.split('/') {
            if !push(s) {
                return None;
            }
        }
    }
    for s in relative.split('/') {
        if !push(s) {
            return None;
        }
    }
    core::str::from_utf8(&output[..cursor]).ok()
}
