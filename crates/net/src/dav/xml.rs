//! Bounded namespace-aware DAV XML. DTDs and unknown entities are refused.
use quick_xml::escape::resolve_xml_entity;
use quick_xml::{NsReader, XmlVersion, events::Event, name::ResolveResult};

pub const D: &str = "DAV:";
pub const C: &str = "urn:ietf:params:xml:ns:caldav";
pub const A: &str = "urn:ietf:params:xml:ns:carddav";
#[derive(Default, Debug)]
pub struct Node {
    pub ns: String,
    pub name: String,
    pub attr_name: String,
    pub text: String,
    pub children: Vec<Node>,
}
impl Node {
    pub fn is(&self, ns: &str, name: &str) -> bool {
        self.ns == ns && self.name == name
    }
    pub fn child(&self, ns: &str, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.is(ns, name))
    }
    pub fn value(&self, ns: &str, name: &str) -> &str {
        self.child(ns, name).map_or("", |n| n.text.trim())
    }
    pub fn has(&self, ns: &str, name: &str) -> bool {
        self.is(ns, name) || self.children.iter().any(|n| n.has(ns, name))
    }
}
pub fn parse(bytes: &[u8]) -> Result<Node, String> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("DAV XML exceeds size limit".into());
    }
    let mut reader = NsReader::from_reader(bytes);
    let mut stack = vec![Node::default()];
    let mut count = 0usize;
    loop {
        let decoder = reader.decoder();
        let (ns, event) = reader.read_resolved_event().map_err(|_| "Malformed DAV XML")?;
        let ns = match ns {
            ResolveResult::Bound(ns) => String::from_utf8_lossy(ns.as_ref()).into_owned(),
            ResolveResult::Unbound => String::new(),
            _ => return Err("Unknown DAV XML namespace".into()),
        };
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                count += 1;
                if count > 200_000 || stack.len() > 32 {
                    return Err("DAV XML exceeds structural limits".into());
                }
                let mut attr_name = String::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|_| "Malformed DAV XML attribute")?;
                    if attr.key.as_ref() == b"name" {
                        attr_name = attr
                            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                            .map_err(|_| "Invalid DAV component name")?
                            .into_owned();
                    }
                }
                let node = Node { ns, name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(), attr_name, ..Default::default() };
                if empty {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    }
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err("Unbalanced DAV XML".into());
                }
                if let Some(node) = stack.pop()
                    && let Some(parent) = stack.last_mut()
                {
                    parent.children.push(node);
                }
            }
            Event::Text(e) => {
                let t = e.xml_content(XmlVersion::Implicit1_0).map_err(|_| "Invalid DAV XML text")?;
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&t);
                }
            }
            Event::CData(e) => {
                let t = e.xml_content(XmlVersion::Implicit1_0).map_err(|_| "Invalid DAV XML CDATA")?;
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&t);
                }
            }
            Event::GeneralRef(e) => {
                let t = if let Some(c) = e.resolve_char_ref().map_err(|_| "Invalid DAV XML reference")? {
                    c.to_string()
                } else {
                    resolve_xml_entity(&e.decode().map_err(|_| "Invalid DAV XML entity")?).ok_or("Unknown DAV XML entity")?.into()
                };
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&t);
                }
            }
            Event::DocType(_) => return Err("DAV XML DTD is forbidden".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err("Truncated DAV XML".into());
    }
    let mut root = stack.pop().ok_or("Empty DAV XML")?;
    if root.children.len() != 1 {
        return Err("DAV XML needs one root".into());
    }
    let root = root.children.remove(0);
    if !root.is(D, "multistatus") {
        return Err("DAV response is not a multistatus".into());
    }
    Ok(root)
}
/// Only successful propstats contribute properties. A missing property is not a deletion.
pub fn properties(response: &Node) -> Vec<&Node> {
    response
        .children
        .iter()
        .filter(|n| n.is(D, "propstat") && n.value(D, "status").split_whitespace().nth(1) == Some("200"))
        .filter_map(|n| n.child(D, "prop"))
        .flat_map(|n| n.children.iter())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn namespaces_entities_and_property_failure() {
        let root=parse(br#"<x:multistatus xmlns:x="DAV:" xmlns:evil="wrong"><x:response><x:href>/a&amp;b</x:href><x:propstat><x:prop><evil:getetag>bad</evil:getetag></x:prop><x:status>HTTP/1.1 404 Not Found</x:status></x:propstat><x:propstat><x:prop><x:getetag>&quot;v1&quot;</x:getetag></x:prop><x:status>HTTP/1.1 200 OK</x:status></x:propstat></x:response></x:multistatus>"#).unwrap();
        let row = &root.children[0];
        assert_eq!(row.value(D, "href"), "/a&b");
        let props = properties(row);
        assert_eq!(props.len(), 1);
        assert!(props[0].is(D, "getetag"));
        assert_eq!(props[0].text, "\"v1\"");
    }
    #[test]
    fn refuses_dtd_and_truncation() {
        assert!(parse(b"<!DOCTYPE x [<!ENTITY a SYSTEM 'file:///etc/passwd'>]><multistatus xmlns='DAV:'/>").is_err());
        assert!(parse(b"<multistatus xmlns='DAV:'><response>").is_err());
    }
}
