//! Model policy: the single source of truth for model-tier pins, default
//! fallback chains, and tier cost rates. Every consumer — synthesis policy,
//! classification chains, budget estimates, and the Action's manifest-defaults
//! mirror — reads from here instead of hardcoding literals independently.
//! Independent hardcoding is exactly how `openai/gpt-4o-mini` and
//! `anthropic/claude-sonnet-4` went stale without anyone noticing. When a pin
//! needs to move, update it once, here, and bump the review date.
//! See Powder card landmark-013.

use crate::*;

pub(crate) fn policy_default_model(policy: Option<&str>) -> Option<String> {
    let tier = match policy.and_then(trimmed_option).as_deref() {
        Some("off") => "off",
        Some("cheap") => "cheap",
        Some("rich") => "rich",
        _ => "balanced",
    };
    Some(default_model_for_tier(tier).into())
}

/// Single source of truth for model-tier pins.
pub(crate) fn default_model_for_tier(tier: &str) -> &'static str {
    match tier {
        "off" => "off",
        // model pin reviewed: 2026-08
        "cheap" | "balanced" => "deepseek/deepseek-v4-flash-0731",
        // model pin reviewed: 2026-08
        "rich" => "deepseek/deepseek-v4-pro-0813",
        // model pin reviewed: 2026-08
        "classification" => "deepseek/deepseek-v4-flash-0731",
        // model pin reviewed: 2026-08
        "classification-fallback" => "google/gemini-3.7-flash",
        // model pin reviewed: 2026-08
        _ => "deepseek/deepseek-v4-flash-0731",
    }
}

/// Default synthesis fallback chain: preference order over the pinned tiers
/// (commodity flash, provider-diverse Gemini, rich escalation) minus the
/// resolved primary. Defined here so portable CLI synthesis and the GitHub
/// Action mirror one chain contract; explicit args or manifest fallbacks win.
pub(crate) fn default_fallback_models(primary: &str) -> String {
    if primary == "off" {
        return String::new();
    }
    let mut models = Vec::new();
    for tier in ["cheap", "classification-fallback", "rich"] {
        let model = default_model_for_tier(tier);
        if model != primary {
            push_unique_model(&mut models, model);
        }
    }
    models.join(",")
}

pub(crate) fn cheap_model(config: &EffectiveSynthesisConfig) -> String {
    if config.model_explicit && config.model != "off" && !config.model.trim().is_empty() {
        config.model.clone()
    } else {
        default_model_for_tier("cheap").into()
    }
}

pub(crate) fn rich_model(config: &EffectiveSynthesisConfig) -> String {
    if config.model_explicit && config.model != "off" && !config.model.trim().is_empty() {
        config.model.clone()
    } else {
        default_model_for_tier("rich").into()
    }
}

pub(crate) fn estimate_model_cost_usd(tier: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let (input_per_million, output_per_million) = match tier {
        "rich" => (1.19, 3.56),
        "off" => (0.0, 0.0),
        // cheap and balanced both pin deepseek-v4-flash-0731
        _ => (0.08, 0.18),
    };
    ((input_tokens as f64 / 1_000_000.0) * input_per_million)
        + ((output_tokens as f64 / 1_000_000.0) * output_per_million)
}
