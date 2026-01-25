use plist_macro::{plist_to_xml_bytes, plist_value_to_xml_bytes};

pub fn plist_to_xml_string(p: &plist::Dictionary) -> String {
    String::from_utf8(plist_to_xml_bytes(p)).unwrap()
}

pub fn plist_value_to_xml_string(p: &plist::Value) -> String {
    String::from_utf8(plist_value_to_xml_bytes(p)).unwrap()
}
