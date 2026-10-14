use owo_colors::OwoColorize;

pub fn success(text: &str) -> String {
    text.green().to_string()
}

pub fn error(text: &str) -> String {
    text.red().to_string()
}

pub fn muted(text: &str) -> String {
    text.dimmed().to_string()
}
