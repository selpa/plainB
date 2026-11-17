use url::Url;

pub fn normalize(input: &str) -> Result<String, &'static str> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        return Err("공백 없는 웹 주소를 입력해 주세요. 예: example.com");
    }

    // A hostname without a scheme uses HTTPS. Explicit schemes are never rewritten.
    let candidate = if input.contains("://") {
        input.to_owned()
    } else {
        let host = input.split('/').next().unwrap_or_default();
        let local = host == "localhost"
            || host.starts_with("localhost:")
            || host == "127.0.0.1"
            || host.starts_with("127.0.0.1:")
            || host == "[::1]"
            || host.starts_with("[::1]:");
        // Reject inputs such as javascript:alert(1), while permitting host:port.
        if let Some((_, port)) = host.rsplit_once(':')
            && !local
            && !port.chars().all(|c| c.is_ascii_digit())
        {
            return Err("http 또는 https 웹 주소만 열 수 있습니다.");
        }
        format!("{}://{input}", if local { "http" } else { "https" })
    };

    let url = Url::parse(&candidate).map_err(|_| "웹 주소 형식을 확인해 주세요.")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("http 또는 https 웹 주소만 열 수 있습니다.");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("사용자 정보가 포함된 주소는 지원하지 않습니다.");
    }
    Ok(url.into())
}

pub fn allowed_navigation(address: &str) -> bool {
    address == "about:blank"
        || Url::parse(address).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
}

/// Only a real user gesture may convert a new-window request to current-window navigation.
pub fn clicked_link_target(address: &str, user_initiated: bool) -> Result<String, &'static str> {
    if !user_initiated {
        return Err("자동 팝업을 차단했습니다.");
    }
    let url = Url::parse(address).map_err(|_| "링크 주소를 확인해 주세요.")?;
    if !matches!(url.scheme(), "http" | "https") || !allowed_navigation(address) {
        return Err("이 버전은 http·https 웹 링크만 엽니다.");
    }
    Ok(url.into())
}

/// Compare full hostnames, including subdomains, ignoring scheme, port and path.
fn site_key(address: &str) -> Option<String> {
    let url = Url::parse(address).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    Some(host)
}

pub fn different_site(source: &str, target: &str) -> bool {
    match (site_key(source), site_key(target)) {
        (Some(source), Some(target)) => source != target,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_comparison_respects_complete_hostnames() {
        for (a, b) in [
            (
                "https://news.example.com/a",
                "http://news.example.com:8080/b",
            ),
            ("http://localhost:8000", "http://localhost:9000"),
            ("https://EXAMPLE.COM.", "https://example.com"),
        ] {
            assert!(!different_site(a, b), "same site: {a} {b}");
        }
        for (a, b) in [
            ("https://news.example.com/a", "https://www.example.com/b"),
            ("https://example.com", "https://sub.example.com"),
            ("https://news.example.co.kr", "https://www.example.co.kr"),
            ("https://example.com", "https://example.net"),
            ("https://one.co.kr", "https://two.co.kr"),
            ("https://one.github.io", "https://two.github.io"),
            ("http://127.0.0.1", "http://127.0.0.2"),
            ("http://127.0.0.1", "http://192.168.0.1"),
        ] {
            assert!(different_site(a, b), "different site: {a} {b}");
        }
        assert!(!different_site("about:blank", "https://example.com"));
    }

    #[test]
    fn opens_clicked_web_links_but_rejects_automatic_popups_and_non_web_targets() {
        assert_eq!(
            clicked_link_target("https://example.com/article", true),
            Ok("https://example.com/article".into())
        );
        assert!(clicked_link_target("https://example.com/article", false).is_err());
        for url in [
            "about:blank",
            "javascript:alert(1)",
            "file:///C:/test",
            "mailto:user@example.com",
            "https://user:secret@example.com",
        ] {
            assert!(clicked_link_target(url, true).is_err());
        }
    }

    #[test]
    fn accepts_web_addresses_and_local_development_servers() {
        for (input, expected) in [
            (" example.com ", "https://example.com/"),
            ("http://example.com/a?q=1#b", "http://example.com/a?q=1#b"),
            ("localhost:8080/test", "http://localhost:8080/test"),
            ("127.0.0.1:8080", "http://127.0.0.1:8080/"),
            ("[::1]:8080", "http://[::1]:8080/"),
            ("example.com:8443", "https://example.com:8443/"),
        ] {
            assert_eq!(normalize(input), Ok(expected.to_owned()));
        }
    }

    #[test]
    fn rejects_non_web_commands_and_ambiguous_addresses() {
        for input in [
            "",
            "search words",
            "javascript:alert(1)",
            "data:text/html,test",
            "file:///C:/Windows/win.ini",
            "ftp://example.com",
            "https://",
            "https://user:secret@example.com",
            "https://example.com:99999",
        ] {
            assert!(normalize(input).is_err(), "unexpectedly accepted: {input}");
        }
    }

    #[test]
    fn remote_navigation_cannot_open_local_files_or_custom_protocols() {
        assert!(allowed_navigation("about:blank"));
        assert!(allowed_navigation("https://example.com/path#section"));
        for address in [
            "file:///C:/test",
            "javascript:alert(1)",
            "ms-settings:",
            "data:text/html,test",
            "https://user@example.com",
        ] {
            assert!(!allowed_navigation(address));
        }
    }
}
