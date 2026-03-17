use owo_colors::OwoColorize;

use crate::service::rm::RmResult;

pub fn render(result: &RmResult) -> String {
    [
        format!(
            "{} {} {}",
            "memento".bold(),
            "rm".cyan().bold(),
            "resource untracked".green()
        ),
        format!("{} {}", "uri".dimmed(), result.uri.cyan()),
        format!("{} {}", "path".dimmed(), result.path.cyan()),
    ]
    .join("\n")
}
