use owo_colors::OwoColorize;

use crate::service::doctor::DoctorResult;

pub fn render(result: &DoctorResult) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        "memento".bold(),
        "doctor".cyan().bold(),
        "environment report".green()
    )];

    for check in &result.checks {
        let status = match check.status.as_str() {
            "ok" => "ok".green().to_string(),
            _ => "missing".yellow().to_string(),
        };
        lines.push(format!(
            "{} {} {}",
            check.label.dimmed(),
            status,
            check.path
        ));
    }

    if result.config_status.ok {
        lines.push(format!(
            "{} ok {}",
            "config_load".dimmed(),
            result.config_status.message
        ));
    } else {
        lines.push(format!(
            "{} error {}",
            "config_load".dimmed(),
            result.config_status.message
        ));
    }

    lines.push(format!(
        "{} {}",
        "item_count".dimmed(),
        result.item_count_status
    ));
    lines.push(format!(
        "{} {}",
        "workspace_embedding".dimmed(),
        result.workspace_embedding_status
    ));
    lines.push(format!(
        "{} {}",
        "active_embedding".dimmed(),
        result.active_embedding_status
    ));
    lines.push(format!(
        "{} {}",
        "test_embedding".dimmed(),
        result.test_embedding_status
    ));

    lines.join("\n")
}
