use owo_colors::OwoColorize;

use crate::service::doctor::DoctorResult;

pub fn render(result: &DoctorResult) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        "memento".bold(),
        "doctor".cyan().bold(),
        "workspace health report".green()
    )];

    for check in &result.checks {
        let status = match check.status.as_str() {
            "ok" => "ok".green().to_string(),
            "warn" => "warn".yellow().to_string(),
            "info" => "info".blue().to_string(),
            _ => "fail".red().to_string(),
        };
        lines.push(format!(
            "{} {} {}",
            status,
            check.label.dimmed(),
            check.detail
        ));
    }

    lines.join("\n")
}
