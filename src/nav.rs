use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use quick_xml::{Reader, events::Event};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NavItem {
    pub title: String,
    pub href: String,
}

pub(crate) fn parse_toc_ncx(content: &[u8]) -> Vec<NavItem> {
    let mut reader = Reader::from_reader(content);
    let mut items = Vec::new();
    let mut buf = Vec::new();

    let mut current_title = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"text" => in_text = true,
            Ok(Event::Text(e)) if in_text => {
                let decoded = reader.decoder().decode(e.as_ref()).unwrap_or_default();
                current_title = decoded.into_owned();
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"text" => in_text = false,
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"content" => {
                if let Some(attr) = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.as_ref() == b"src")
                {
                    let href = attr
                        .decode_and_unescape_value(reader.decoder())
                        .unwrap_or_else(|_| {
                            reader.decoder().decode(&attr.value).unwrap_or_default()
                        })
                        .into_owned();

                    items.push(NavItem {
                        title: current_title.trim().to_string(),
                        href,
                    });
                }
            }
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }
    items
}

pub(crate) fn parse_nav_xhtml(content: &[u8]) -> Vec<NavItem> {
    let mut reader = Reader::from_reader(content);
    let mut items = Vec::new();
    let mut buf = Vec::new();
    let mut in_link = false;
    let mut current_href = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"a" => {
                if let Some(attr) = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.as_ref() == b"href")
                {
                    current_href = attr
                        .decode_and_unescape_value(reader.decoder())
                        .unwrap_or_else(|_| {
                            reader.decoder().decode(&attr.value).unwrap_or_default()
                        })
                        .into_owned();
                    in_link = true;
                }
            }
            Ok(Event::Text(e)) if in_link => {
                let decoded = reader.decoder().decode(e.as_ref()).unwrap_or_default();
                let title = decoded.trim().to_string();

                items.push(NavItem {
                    title,
                    href: current_href.clone(),
                });
                in_link = false;
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"a" => in_link = false,
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }
    items
}
