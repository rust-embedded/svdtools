pub fn unwrap_or_empty_str(opt_str: &Option<String>) -> &str {
    match opt_str {
        Some(desc) => desc,
        None => "",
    }
}

pub fn get_description(opt_str: &Option<String>) -> String {
    let desc: &str = unwrap_or_empty_str(opt_str);

    // remove duplicate whitespaces
    let words: Vec<&str> = desc.split_whitespace().collect();

    words.join(" ")
}

/// Make everything uppercase except first two character, which should be "0x"
pub fn format_address(hex_address: u64) -> String {
    let addr = format! {"{:x}", hex_address};
    let addr = addr.to_uppercase();
    format!("0x{}", addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correctly_format_address() {
        let addr: u32 = 0xde4dBeeF;
        let formatted_addr = format_address(addr as u64);
        assert_eq!(formatted_addr, "0xDE4DBEEF");
    }
}
