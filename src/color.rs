pub fn colored_str(content: &str, color: &str) -> String {
    format!("{}{}{}{}%f", "%F{", color, "}", content)
}

pub fn bold_str(content: &str) -> String {
    format!("%B{}%b", content)
}
