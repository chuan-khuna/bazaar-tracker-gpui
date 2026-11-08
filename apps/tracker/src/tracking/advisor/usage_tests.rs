use serde_json::json;

use super::*;

fn stats(cost_usd: f64, tool_calls: u32) -> Stats {
    Stats::from_json(&json!({
        "model": "fake/advisor",
        "elapsedMs": 4210,
        "tokens": {"input": 12034, "output": 210, "cacheRead": 9000, "cacheWrite": 0, "total": 12244},
        "costUsd": cost_usd,
        "pricingSource": "provider",
        "toolCalls": tool_calls,
    }))
    .unwrap()
}

#[test]
fn the_line_is_model_time_tokens_cost_and_tools() {
    assert_eq!(stats(0.028512, 3).line(), ["fake/advisor", "4.2 s", "12,244 tokens", "$0.0285", "3 tool calls"]);
    assert_eq!(stats(0.0, 1).line(), ["fake/advisor", "4.2 s", "12,244 tokens", "free", "1 tool call"]);
    // No tool calls: no tools part.
    assert_eq!(stats(1.5, 0).line().len(), 4);
}

#[test]
fn the_tooltip_breaks_it_down() {
    assert_eq!(
        stats(0.028512, 3).breakdown(),
        [
            "Input: 12,034 tokens",
            "Output: 210 tokens",
            "Cache: 9,000 read, 0 written",
            "Cost: $0.0285 (estimate, provider-reported)",
            "Time: 4.2 s, including tool calls",
        ]
    );
}

#[test]
fn missing_fields_are_zero_and_a_non_object_is_none() {
    let read = Stats::from_json(&json!({"model": "m"})).unwrap();
    assert_eq!(read.tokens.total, 0);
    assert_eq!(read.line(), ["m", "0 ms", "0 tokens", "free"]);
    assert_eq!(Stats::from_json(&json!(null)), None);
}

#[test]
fn dollars_have_three_significant_digits() {
    assert_eq!(usd(0.0), "$0");
    assert_eq!(usd(0.028512), "$0.0285");
    assert_eq!(usd(0.1), "$0.1");
    assert_eq!(usd(0.09996), "$0.1");
    assert_eq!(usd(1.5), "$1.5");
    assert_eq!(usd(12.345), "$12.3");
    assert_eq!(usd(1234.5), "$1230");
    assert_eq!(usd(0.000012345), "$0.0000123");
}

#[test]
fn durations_and_counts_read_as_in_tau_app() {
    assert_eq!(duration(850.4), "850 ms");
    assert_eq!(duration(4210.0), "4.2 s");
    assert_eq!(duration(65_000.0), "1 min 5 s");
    assert_eq!(count(0), "0");
    assert_eq!(count(999), "999");
    assert_eq!(count(1234), "1,234");
    assert_eq!(count(1_234_567), "1,234,567");
}
