use std::{collections::BTreeSet, path::PathBuf};

pub fn host(address: &str) -> Option<String> {
    if !crate::address::allowed_navigation(address) {
        return None;
    }
    let url = url::Url::parse(address).ok()?;
    Some(url.host_str()?.trim_end_matches('.').to_ascii_lowercase())
}

pub struct Permissions {
    path: PathBuf,
    allowed: BTreeSet<String>,
}

impl Permissions {
    pub fn load(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let allowed = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self { path, allowed })
    }

    pub fn allows(&self, host: &str) -> bool {
        self.allowed.contains(host)
    }

    pub fn set(&mut self, host: String, allow: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut next = self.allowed.clone();
        if allow {
            next.insert(host);
        } else {
            next.remove(&host);
        }
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&next)?)?;
        std::fs::rename(&temporary, &self.path)?;
        self.allowed = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_are_persistent_and_can_be_revoked() {
        let path =
            std::env::temp_dir().join(format!("plain-popup-test-{}.json", std::process::id()));
        let mut permissions = Permissions::load(path.clone()).unwrap();
        permissions.set("news.example.com".into(), true).unwrap();
        let mut permissions = Permissions::load(path.clone()).unwrap();
        assert!(permissions.allows("news.example.com"));
        assert!(!permissions.allows("www.example.com"));
        permissions.set("news.example.com".into(), false).unwrap();
        assert!(
            !Permissions::load(path.clone())
                .unwrap()
                .allows("news.example.com")
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn host_is_normalized_and_non_web_requests_are_rejected() {
        assert_eq!(
            host("https://NEWS.Example.com.:8443/a"),
            Some("news.example.com".into())
        );
        assert_eq!(host("javascript:alert(1)"), None);
        assert_eq!(host("https://user:secret@example.com"), None);
    }
}
