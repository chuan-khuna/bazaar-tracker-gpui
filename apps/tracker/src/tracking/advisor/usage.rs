//! A reply's usage, from its `stats`, and the usage line under it as tau-app's chat writes it
//! (`ResponseStats.tsx`, `lib/format.ts`): model · time · tokens · cost · tool calls, with the
//! breakdown in a tooltip.

use serde::Deserialize;
use serde_json::Value;

/// `{model, elapsedMs, tokens, costUsd, pricingSource, toolCalls}`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Stats {
    pub model: String,
    pub elapsed_ms: f64,
    pub tokens: Tokens,
    pub cost_usd: f64,
    pub pricing_source: String,
    pub tool_calls: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total: u64,
}

impl Stats {
    /// From the `stats` object; none when it isn't one.
    pub fn from_json(stats: &Value) -> Option<Self> {
        if !stats.is_object() {
            return None;
        }
        serde_json::from_value(stats.clone()).ok()
    }

    /// The line's parts: model, time, tokens, cost (`free` at 0), and the tool calls when there were any.
    pub fn line(&self) -> Vec<String> {
        let mut parts = vec![self.model.clone(), duration(self.elapsed_ms), format!("{} tokens", count(self.tokens.total))];
        parts.push(if self.cost_usd == 0.0 { "free".to_owned() } else { usd(self.cost_usd) });
        if self.tool_calls > 0 {
            parts.push(format!("{} tool {}", self.tool_calls, if self.tool_calls == 1 { "call" } else { "calls" }));
        }
        parts
    }

    /// The tooltip: input, output, cache read and written, cost, time.
    pub fn breakdown(&self) -> Vec<String> {
        let source = if self.pricing_source == "openrouter" { "live prices" } else { "provider-reported" };
        vec![
            format!("Input: {} tokens", count(self.tokens.input)),
            format!("Output: {} tokens", count(self.tokens.output)),
            format!("Cache: {} read, {} written", count(self.tokens.cache_read), count(self.tokens.cache_write)),
            format!("Cost: {} (estimate, {source})", usd(self.cost_usd)),
            format!("Time: {}, including tool calls", duration(self.elapsed_ms)),
        ]
    }
}

/// US dollars to three significant digits: `$0.0285`, `$1.5`, `$1230`; zero is `$0`.
pub fn usd(amount: f64) -> String {
    if amount == 0.0 || !amount.is_finite() {
        return "$0".to_owned();
    }
    let magnitude = amount.abs().log10().floor() as i32;
    let scale = 10f64.powi(magnitude - 2);
    let rounded = (amount / scale).round() * scale;
    let decimals = (2 - magnitude).max(0) as usize;
    let text = format!("{rounded:.decimals$}");
    let text = if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text };
    format!("${text}")
}

/// `850 ms`, `4.2 s`, `1 min 5 s`.
pub fn duration(ms: f64) -> String {
    if ms < 1000.0 {
        return format!("{} ms", ms.round() as i64);
    }
    if ms < 60_000.0 {
        return format!("{:.1} s", ms / 1000.0);
    }
    let seconds = (ms / 1000.0).round() as i64;
    format!("{} min {} s", seconds / 60, seconds % 60)
}

/// `1,234`.
pub fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
