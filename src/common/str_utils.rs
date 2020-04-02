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

fn get_access(opt_str: &Option<String>) -> &str {
    let access = unwrap_or_empty_str(opt_str);

    match access {
        "read-only" => "ro",
        "read-write" => "rw",
        "write-only" => "wo",
        other => other,
    }
}

pub fn access_with_brace(opt_str: &Option<String>) -> String {
    let access = get_access(opt_str);
    if access.is_empty() {
        access.to_string()
    } else {
        format!(" ({})", access)
    }
}

/// Make everything uppercase except first two character, which should be "0x"
pub fn format_address(hex_address: &str) -> String {
    assert_eq!(&hex_address[0..2], "0x");
    format!("0x{}", hex_address[2..].to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correctly_format_address() {
        let addr = "0xdeadBeeF";
        let formatted_addr = format_address(addr);
        assert_eq!(formatted_addr, "0xDEADBEEF");
    }
}
