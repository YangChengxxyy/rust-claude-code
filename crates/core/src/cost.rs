use crate::message::Usage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_read_per_million: f64,
    pub cache_creation_per_million: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetStatus {
    Ok,
    Warning { remaining_usd: f64 },
    Exceeded { over_usd: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CostRecord {
    pub model: String,
    pub usage: Usage,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CostTracker {
    records: Vec<CostRecord>,
    total_cost_usd: f64,
    max_budget_usd: Option<f64>,
}

impl ModelPricing {
    fn cost_for_tokens(tokens: u32, per_million: f64) -> f64 {
        (tokens as f64 / 1_000_000.0) * per_million
    }
}

pub fn get_pricing(model: &str) -> ModelPricing {
    let model = model.to_ascii_lowercase();
    if model.contains("opus") {
        ModelPricing {
            input_per_million: 15.0,
            output_per_million: 75.0,
            cache_read_per_million: 1.5,
            cache_creation_per_million: 18.75,
        }
    } else if model.contains("haiku") {
        ModelPricing {
            input_per_million: 0.25,
            output_per_million: 1.25,
            cache_read_per_million: 0.03,
            cache_creation_per_million: 0.3,
        }
    } else {
        ModelPricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cache_read_per_million: 0.3,
            cache_creation_per_million: 3.75,
        }
    }
}

pub fn pricing_family(model: &str) -> &'static str {
    let model = model.to_ascii_lowercase();
    if model.contains("opus") {
        "opus"
    } else if model.contains("haiku") {
        "haiku"
    } else {
        "sonnet"
    }
}

pub fn calculate_cost(usage: &Usage, model: &str) -> f64 {
    let pricing = get_pricing(model);
    ModelPricing::cost_for_tokens(usage.input_tokens, pricing.input_per_million)
        + ModelPricing::cost_for_tokens(usage.output_tokens, pricing.output_per_million)
        + ModelPricing::cost_for_tokens(
            usage.cache_read_input_tokens,
            pricing.cache_read_per_million,
        )
        + ModelPricing::cost_for_tokens(
            usage.cache_creation_input_tokens,
            pricing.cache_creation_per_million,
        )
}

impl CostTracker {
    pub fn new(max_budget_usd: Option<f64>) -> Self {
        Self {
            records: Vec::new(),
            total_cost_usd: 0.0,
            max_budget_usd,
        }
    }

    pub fn from_total(total_cost_usd: f64, max_budget_usd: Option<f64>) -> Self {
        Self {
            records: Vec::new(),
            total_cost_usd,
            max_budget_usd,
        }
    }

    pub fn record_usage(&mut self, usage: Usage, model: impl Into<String>) -> f64 {
        let model = model.into();
        let cost_usd = calculate_cost(&usage, &model);
        self.total_cost_usd += cost_usd;
        self.records.push(CostRecord {
            model,
            usage,
            cost_usd,
        });
        cost_usd
    }

    pub fn records(&self) -> &[CostRecord] {
        &self.records
    }

    pub fn total_cost_usd(&self) -> f64 {
        self.total_cost_usd
    }

    pub fn max_budget_usd(&self) -> Option<f64> {
        self.max_budget_usd
    }

    pub fn check_budget(&self) -> BudgetStatus {
        let Some(max_budget_usd) = self.max_budget_usd else {
            return BudgetStatus::Ok;
        };
        if self.total_cost_usd > max_budget_usd {
            BudgetStatus::Exceeded {
                over_usd: self.total_cost_usd - max_budget_usd,
            }
        } else {
            let remaining_usd = max_budget_usd - self.total_cost_usd;
            if max_budget_usd > 0.0 && remaining_usd <= max_budget_usd * 0.1 {
                BudgetStatus::Warning { remaining_usd }
            } else {
                BudgetStatus::Ok
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage() -> Usage {
        Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
        }
    }

    #[test]
    fn selects_model_pricing() {
        assert_eq!(get_pricing("claude-opus-4-7").input_per_million, 15.0);
        assert_eq!(get_pricing("claude-sonnet-4-6").input_per_million, 3.0);
        assert_eq!(get_pricing("claude-haiku-4-5").input_per_million, 0.25);
        assert_eq!(get_pricing("unknown-model").input_per_million, 3.0);
    }

    #[test]
    fn calculates_cache_aware_cost() {
        let cost = calculate_cost(&usage(), "claude-sonnet-4-6");
        assert!((cost - 22.05).abs() < f64::EPSILON);
    }

    #[test]
    fn tracks_total_cost() {
        let mut tracker = CostTracker::new(None);
        let first = tracker.record_usage(usage(), "claude-haiku-4-5");
        let second = tracker.record_usage(usage(), "claude-haiku-4-5");
        assert_eq!(tracker.records().len(), 2);
        assert!((tracker.total_cost_usd() - first - second).abs() < f64::EPSILON);
    }

    #[test]
    fn reports_budget_status() {
        assert_eq!(
            CostTracker::from_total(2.0, None).check_budget(),
            BudgetStatus::Ok
        );
        assert_eq!(
            CostTracker::from_total(11.0, Some(10.0)).check_budget(),
            BudgetStatus::Exceeded { over_usd: 1.0 }
        );
        assert_eq!(
            CostTracker::from_total(9.5, Some(10.0)).check_budget(),
            BudgetStatus::Warning { remaining_usd: 0.5 }
        );
    }
}
