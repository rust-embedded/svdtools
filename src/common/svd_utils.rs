use svd_parser::Access;

fn access_str(access: &Option<Access>) -> &str {
    match access {
        None => "",
        Some(access) => match access {
            Access::ReadOnly => "ro",
            Access::ReadWrite => "rw",
            Access::ReadWriteOnce => "rwonce",
            Access::WriteOnce => "wonce",
            Access::WriteOnly => "wo",
        },
    }
}

pub fn access_with_brace(access: &Option<Access>) -> String {
    let access = access_str(access);
    if access.is_empty() {
        access.to_string()
    } else {
        format!(" ({})", access)
    }
}
