use adblock::{
    Engine,
    lists::{FilterSet, ParseOptions, RuleTypes},
    request::Request,
};
use std::cell::Cell;

/// Network-only filtering. Publisher HTML and same-site requests are preserved.
pub struct AdBlocker {
    engine: Engine,
    enabled: Cell<bool>,
    blocked: Cell<u64>,
}

impl Default for AdBlocker {
    fn default() -> Self {
        Self::from_rules(include_str!("../filters/easylist.txt"))
    }
}

impl AdBlocker {
    pub fn from_rules(rules: &str) -> Self {
        let mut filters = FilterSet::new(false);
        filters.add_filter_list(
            rules.to_owned(),
            ParseOptions {
                rule_types: RuleTypes::NetworkOnly,
                ..Default::default()
            },
        );
        Self {
            engine: Engine::new_with_filter_set(filters),
            enabled: Cell::new(true),
            blocked: Cell::new(0),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.get()
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }
    pub fn blocked(&self) -> u64 {
        self.blocked.get()
    }
    pub fn record_block(&self) {
        self.blocked.set(self.blocked.get().saturating_add(1));
    }

    pub fn should_block(&self, url: &str, source: &str, kind: &str, method: &str) -> bool {
        // Never block top-level navigation, modify HTML, or infer ads from DOM names.
        if !self.enabled() || kind == "document" || source.is_empty() {
            return false;
        }
        let Ok(request) = Request::new(url, source, kind, method) else {
            return false;
        };
        if !request.is_supported || !request.is_third_party {
            return false;
        }
        self.engine.check_network_request(&request).should_block()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: &str = "||ads.example^\n@@||ads.example/allowed.js$script\n/advert/$script\nnews.example##.sponsor";

    #[test]
    fn blocks_matching_third_party_requests_but_respects_exceptions() {
        let blocker = AdBlocker::from_rules(RULES);
        assert!(blocker.should_block(
            "https://ads.example/banner.js",
            "https://news.example/",
            "script",
            "GET"
        ));
        assert!(!blocker.should_block(
            "https://ads.example/allowed.js",
            "https://news.example/",
            "script",
            "GET"
        ));
        assert!(!blocker.should_block(
            "https://cdn.example/content.js",
            "https://news.example/",
            "script",
            "GET"
        ));
    }

    #[test]
    fn preserves_publisher_resources_subdomains_and_navigation() {
        let blocker = AdBlocker::from_rules(RULES);
        for url in [
            "https://news.example/advert/sponsor.js",
            "https://cdn.news.example/advert/sponsor.js",
        ] {
            assert!(!blocker.should_block(url, "https://news.example/", "script", "GET"));
        }
        assert!(!blocker.should_block(
            "https://ads.example/",
            "https://news.example/",
            "document",
            "GET"
        ));
        assert!(!blocker.should_block("https://ads.example/banner.js", "", "script", "GET"));
    }

    #[test]
    fn toggle_disables_and_reenables_blocking() {
        let blocker = AdBlocker::from_rules(RULES);
        blocker.set_enabled(false);
        assert!(!blocker.should_block(
            "https://ads.example/banner.js",
            "https://news.example/",
            "script",
            "GET"
        ));
        blocker.set_enabled(true);
        assert!(blocker.should_block(
            "https://ads.example/banner.js",
            "https://news.example/",
            "script",
            "GET"
        ));
    }

    #[test]
    fn bundled_easylist_blocks_known_ad_servers_and_allows_normal_assets() {
        let blocker = AdBlocker::default();
        for url in [
            "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js",
            "https://securepubads.g.doubleclick.net/tag/js/gpt.js",
        ] {
            assert!(
                blocker.should_block(url, "https://publisher.example/", "script", "GET"),
                "not blocked: {url}"
            );
        }
        assert!(!blocker.should_block(
            "https://cdn.jsdelivr.net/npm/example/content.js",
            "https://publisher.example/",
            "script",
            "GET"
        ));
    }
}
