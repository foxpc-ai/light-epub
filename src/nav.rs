use crate::opf::SpineItem;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use quick_xml::{Reader, events::Event};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NavItem {
    pub title: String,
    pub href: String,
    pub spine_index: usize,
    pub children: Vec<NavItem>,
}

fn find_spine_index(href: &str, spine_items: &[SpineItem]) -> usize {
    let base_path = href.split('#').next().unwrap_or(href);

    spine_items
        .iter()
        .position(|item| item.href.ends_with(base_path))
        .unwrap_or(0)
}

pub(crate) fn parse_toc_ncx(content: &[u8], spine_items: &[SpineItem]) -> Vec<NavItem> {
    let mut reader = Reader::from_reader(content);
    let mut buf = Vec::new();

    let mut root_items = Vec::new();
    let mut stack: Vec<NavItem> = Vec::new();
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"navPoint" => {
                stack.push(NavItem {
                    title: String::new(),
                    href: String::new(),
                    spine_index: 0,
                    children: Vec::new(),
                });
            }
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"text" => in_text = true,
            Ok(Event::Text(e)) if in_text => {
                if let Some(item) = stack.last_mut() {
                    let decoded = reader.decoder().decode(e.as_ref()).unwrap_or_default();
                    item.title = decoded.trim().to_string();
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"text" => in_text = false,
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"content" => {
                if let Some(item) = stack.last_mut()
                    && let Some(attr) = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .find(|a| a.key.as_ref() == b"src")
                {
                    let href = attr
                        .decode_and_unescape_value(reader.decoder())
                        .unwrap_or_default()
                        .into_owned();
                    item.spine_index = find_spine_index(&href, spine_items);
                    item.href = href;
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"navPoint" => {
                if let Some(finished_item) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(finished_item);
                    } else {
                        root_items.push(finished_item);
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }
    root_items
}

pub(crate) fn parse_nav_xhtml(content: &[u8], spine_items: &[SpineItem]) -> Vec<NavItem> {
    let mut reader = Reader::from_reader(content);
    let mut buf = Vec::new();

    let mut root_items = Vec::new();
    let mut stack: Vec<Vec<NavItem>> = Vec::new();
    stack.push(Vec::new());

    let mut in_anchor = false;
    let mut current_href = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e))
                if e.local_name().as_ref() == b"ol" || e.local_name().as_ref() == b"ul" =>
            {
                stack.push(Vec::new());
            }
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"a" => {
                if let Some(attr) = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.as_ref() == b"href")
                {
                    current_href = attr
                        .decode_and_unescape_value(reader.decoder())
                        .unwrap_or_default()
                        .into_owned();
                    in_anchor = true;
                }
            }
            Ok(Event::Text(e)) if in_anchor => {
                let title = reader
                    .decoder()
                    .decode(e.as_ref())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let spine_index = find_spine_index(&current_href, spine_items);

                if let Some(current_level) = stack.last_mut() {
                    current_level.push(NavItem {
                        title,
                        href: current_href.clone(),
                        spine_index,
                        children: Vec::new(),
                    });
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"a" => in_anchor = false,
            Ok(Event::End(e))
                if e.local_name().as_ref() == b"ol" || e.local_name().as_ref() == b"ul" =>
            {
                if let Some(children) = stack.pop() {
                    if let Some(parent_level) = stack.last_mut() {
                        if let Some(last_item) = parent_level.last_mut() {
                            last_item.children = children;
                        } else {
                            root_items.extend(children);
                        }
                    } else {
                        root_items.extend(children);
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }

    if root_items.is_empty() && !stack.is_empty() {
        return stack.remove(0);
    }

    root_items
}
