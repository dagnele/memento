use owo_colors::OwoColorize;

use crate::service::models::ModelsResult;

pub fn render(result: &ModelsResult) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        "memento".bold(),
        "models".cyan().bold(),
        "supported embedding models".green()
    )];

    for model in &result.models {
        let mut line = format!(
            "{} {}",
            model.name.cyan(),
            format!("dim={}", model.dimension).dimmed(),
        );

        if model.recommended {
            line.push(' ');
            line.push_str(&"recommended".yellow().to_string());
        }

        line.push(' ');
        line.push_str(&format!("use={}", model.use_case).dimmed().to_string());
        lines.push(line);
    }

    lines.join("\n")
}
