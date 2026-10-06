/// Formats a generation cost in USD for display.
///
/// - Costs at or above one cent (`>= 0.01`) show two decimals (e.g. `$0.03`).
/// - Costs below one cent show three decimals so the significant digit stays
///   visible (e.g. `$0.006`).
/// - Zero or negative costs (free/experimental models, legacy data glitches)
///   render as `$0.00`; callers hide the cost line entirely for `cost <= 0.0`.
pub fn format_cost(cost: f64) -> String {
    if cost <= 0.0 {
        return "$0.00".to_string();
    }
    if cost >= 0.01 {
        format!("${cost:.2}")
    } else {
        format!("${cost:.3}")
    }
}

/// Display string for templates: empty when there is nothing worth showing
/// (free models, failures, legacy data glitches), otherwise [`format_cost`].
pub fn cost_display(cost: f64) -> String {
    if cost > 0.0 {
        format_cost(cost)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_at_or_above_one_cent_use_two_decimals() {
        assert_eq!(format_cost(0.01), "$0.01");
        assert_eq!(format_cost(0.038_797_5), "$0.04");
        assert_eq!(format_cost(1.2), "$1.20");
        assert_eq!(format_cost(0.5), "$0.50");
    }

    #[test]
    fn costs_below_one_cent_use_three_decimals() {
        assert_eq!(format_cost(0.005_578), "$0.006");
        assert_eq!(format_cost(0.009_999), "$0.010");
        assert_eq!(format_cost(0.001), "$0.001");
    }

    #[test]
    fn zero_and_negative_costs_render_as_zero() {
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(-0.25), "$0.00");
    }

    #[test]
    fn display_string_is_empty_for_non_positive_costs() {
        assert_eq!(cost_display(0.0), "");
        assert_eq!(cost_display(-0.25), "");
        assert_eq!(cost_display(0.038_797_5), "$0.04");
        assert_eq!(cost_display(0.005_578), "$0.006");
    }
}
