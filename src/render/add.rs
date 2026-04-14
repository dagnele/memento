use owo_colors::OwoColorize;

use crate::service::add::AddResult;

pub fn render(result: &AddResult) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        "memento".bold(),
        "add".cyan().bold(),
        format!(
            "indexed {} resource(s)",
            result.indexed_paths.len() + result.metadata_only_paths.len()
        )
        .green()
    )];

    for path in &result.indexed_paths {
        lines.push(format!("{} {}", "added".dimmed(), path.cyan()));
    }

    for path in &result.metadata_only_paths {
        lines.push(format!(
            "{} {} {}",
            "added".dimmed(),
            path.cyan(),
            "metadata only".dimmed()
        ));
    }

    for path in &result.skipped_paths {
        lines.push(format!(
            "{} {} {}",
            "skipped".dimmed(),
            path.yellow(),
            "already indexed; use --force to re-add".dimmed()
        ));
    }

    lines.join("\n")
}
