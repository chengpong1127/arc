use anyhow::{Context, Result};
use console::{Style, style};
use dialoguer::{Confirm, Select, theme::ColorfulTheme};

use crate::cli::UsageProfile;

pub fn confirm_install() -> Result<bool> {
    Confirm::with_theme(&theme())
        .with_prompt("Continue with this installation plan?")
        .default(false)
        .interact()
        .context("could not read installation confirmation")
}

pub fn select_usage_profile() -> Result<UsageProfile> {
    let profiles = [UsageProfile::ModelTraining, UsageProfile::CudaDevelopment];
    let options = profiles.map(UsageProfile::selection_label);
    println!(
        "\n  {}\n  {}\n  {}\n",
        style("Set up your GPU environment").cyan().bold(),
        style("Choose the profile that best matches your work.").dim(),
        style("Use ↑/↓ to move, then press Enter.").dim()
    );
    let selection = Select::with_theme(&theme())
        .with_prompt("What will you use CUDA for?")
        .items(&options)
        .default(0)
        .interact()
        .context("could not read the usage profile")?;
    Ok(profiles[selection])
}

pub fn confirm_uninstall() -> Result<bool> {
    Confirm::with_theme(&theme())
        .with_prompt("Continue with this uninstall plan?")
        .default(false)
        .interact()
        .context("could not read uninstall confirmation")
}

pub fn confirm_upgrade() -> Result<bool> {
    Confirm::with_theme(&theme())
        .with_prompt("Continue with this upgrade plan?")
        .default(false)
        .interact()
        .context("could not read upgrade confirmation")
}

fn theme() -> ColorfulTheme {
    ColorfulTheme {
        prompt_style: Style::new().for_stderr().cyan().bold(),
        prompt_prefix: style("◆".to_owned()).for_stderr().cyan(),
        prompt_suffix: style("›".to_owned()).for_stderr().cyan(),
        success_prefix: style("✓".to_owned()).for_stderr().green(),
        active_item_prefix: style("❯".to_owned()).for_stderr().cyan().bold(),
        active_item_style: Style::new().for_stderr().cyan().bold(),
        inactive_item_prefix: style(" ".to_owned()).for_stderr(),
        hint_style: Style::new().for_stderr().dim(),
        values_style: Style::new().for_stderr().green().bold(),
        ..ColorfulTheme::default()
    }
}
