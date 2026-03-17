use owo_colors::OwoColorize;

use crate::service::forget::ForgetResult;

pub fn render(result: &ForgetResult) -> String {
    [
        format!(
            "{} {} {}",
            "memento".bold(),
            "forget".cyan().bold(),
            "item removed".green()
        ),
        format!("{} {}", "uri".dimmed(), result.uri.cyan()),
        format!("{} {}", "path".dimmed(), result.path.cyan()),
    ]
    .join("\n")
}
