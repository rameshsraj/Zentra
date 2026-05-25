use crate::{BreadcrumbEntry, DrilldownContext, DrilldownLevel};

/// Breadcrumb builder — constructs clickable navigation trail
pub struct Breadcrumb;

impl Breadcrumb {
    /// Build a breadcrumb trail from the current drill-down context.
    ///
    /// Example: Global → Network → Host(10.0.0.1) → Server(web-01) → Metric(cpu_usage)
    pub fn build(context: &DrilldownContext) -> Vec<BreadcrumbEntry> {
        let mut trail = Vec::new();

        // Always start with Global
        trail.push(BreadcrumbEntry {
            level: DrilldownLevel::Global,
            entity_id: None,
            label: "Global".into(),
            time_level: context.time_level,
            filters: Default::default(),
        });

        // Add intermediate levels based on the current level
        if context.level != DrilldownLevel::Global {
            // Add the current level
            trail.push(BreadcrumbEntry {
                level: context.level.clone(),
                entity_id: context.entity_id.clone(),
                label: context.entity_label.clone().unwrap_or_else(|| context.level.as_str().into()),
                time_level: context.time_level,
                filters: context.filters.clone(),
            });
        }

        trail
    }

    /// Reconstruct a context from a breadcrumb entry (for "click to navigate back")
    pub fn to_context(entry: &BreadcrumbEntry, time_range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)) -> DrilldownContext {
        DrilldownContext {
            level: entry.level.clone(),
            entity_id: entry.entity_id.clone(),
            entity_label: Some(entry.label.clone()),
            time_level: entry.time_level,
            time_range,
            filters: entry.filters.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeLevel;
    use chrono::Utc;

    #[test]
    fn test_breadcrumb_global() {
        let ctx = DrilldownContext {
            level: DrilldownLevel::Global,
            entity_id: None,
            entity_label: None,
            time_level: TimeLevel::Hour,
            time_range: (Utc::now(), Utc::now()),
            filters: Default::default(),
        };
        let trail = Breadcrumb::build(&ctx);
        assert_eq!(trail.len(), 1);
        assert_eq!(trail[0].label, "Global");
    }

    #[test]
    fn test_breadcrumb_drilldown() {
        let ctx = DrilldownContext {
            level: DrilldownLevel::Server,
            entity_id: Some("web-01".into()),
            entity_label: Some("Web Server 01".into()),
            time_level: TimeLevel::Minute,
            time_range: (Utc::now(), Utc::now()),
            filters: Default::default(),
        };
        let trail = Breadcrumb::build(&ctx);
        assert_eq!(trail.len(), 2);
        assert_eq!(trail[0].label, "Global");
        assert_eq!(trail[1].label, "Web Server 01");
    }
}
