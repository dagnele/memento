use owo_colors::OwoColorize;

use crate::service::remember::RememberResult;

pub fn render(result: &RememberResult) -> String {
    [
        format!(
            "{} {} {}",
            "memento".bold(),
            "remember".cyan().bold(),
            "item stored".green()
        ),
        format!("{} {}", "uri".dimmed(), result.uri.cyan()),
        format!("{} {}", "path".dimmed(), result.path.cyan()),
    ]
    .join("\n")
}
