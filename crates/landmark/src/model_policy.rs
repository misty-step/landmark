//! Model policy: the single source of truth for model-tier pins, default
//! fallback chains, and tier cost rates. Every consumer — synthesis policy,
//! classification chains, budget estimates, and the Action's manifest-defaults
//! mirror — reads from here instead of hardcoding literals independently.
//! Independent hardcoding is exactly how `openai/gpt-4o-mini` and
//! `anthropic/claude-sonnet-4` went stale without anyone noticing. When a pin
//! needs to move, update it once, here, and bump the review date.
//! `bin/check-model-pin-freshness` (run by `bin/gate`) fails any pin whose
//! review date is older than one quarter, so stale pins cannot ship silently.
//! See Powder card landmark-013.

use crate::*;

pub(crate) fn policy_default_model(policy: Option<&str>) -> Option<String> {
    let normalized = policy
        .and_then(trimmed_option)
        .map(|value| value.to_ascii_lowercase());
    let tier = match normalized.as_deref() {
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

/// Models attempted for note synthesis: explicitly configured fallbacks when
/// present, otherwise the derived default chain (preference order over the
/// pinned tiers minus the SELECTED primary — the escalated rich model, not
/// just config.model). This is the only place the derived chain exists, so
/// classification — which reads only configured fallbacks — can never
/// inherit synthesis escalation.
pub(crate) fn effective_fallback_models(
    config: &EffectiveSynthesisConfig,
    selected_primary: &str,
) -> Vec<String> {
    let mut models: Vec<String> = config
        .fallback_models
        .split(',')
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
        .collect();
    if models.is_empty() && selected_primary != "off" {
        for tier in ["cheap", "classification-fallback", "rich"] {
            let model = default_model_for_tier(tier);
            if model != selected_primary {
                push_unique_model(&mut models, model);
            }
        }
    }
    models
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
