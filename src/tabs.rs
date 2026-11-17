pub type TabId = u64;

/// Stable identities prevent queued callbacks from closed pages reaching other tabs.
pub struct Tabs<T> {
    entries: Vec<(TabId, T)>,
    active: Option<TabId>,
    next_id: TabId,
}
impl<T> Default for Tabs<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            active: None,
            next_id: 1,
        }
    }
}
impl<T> Tabs<T> {
    pub fn create<E>(&mut self, factory: impl FnOnce(TabId) -> Result<T, E>) -> Result<TabId, E> {
        let id = self.next_id;
        self.next_id += 1;
        let value = factory(id)?;
        self.entries.push((id, value));
        self.active = Some(id);
        Ok(id)
    }
    pub fn active_id(&self) -> Option<TabId> {
        self.active
    }
    pub fn active(&self) -> Option<&T> {
        self.active.and_then(|id| self.get(id))
    }
    pub fn get(&self, id: TabId) -> Option<&T> {
        self.entries
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, value)| value)
    }
    pub fn get_mut(&mut self, id: TabId) -> Option<&mut T> {
        self.entries
            .iter_mut()
            .find(|(key, _)| *key == id)
            .map(|(_, value)| value)
    }
    pub fn iter(&self) -> impl Iterator<Item = (TabId, &T)> {
        self.entries.iter().map(|(id, value)| (*id, value))
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn select(&mut self, id: TabId) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.active = Some(id);
        true
    }
    /// Move by stable identity, never changing the selected tab or its page.
    pub fn move_before(&mut self, id: TabId, before: Option<TabId>) -> bool {
        if before == Some(id) {
            return false;
        }
        let Some(index) = self.entries.iter().position(|(key, _)| *key == id) else {
            return false;
        };
        if before.is_some_and(|target| self.get(target).is_none()) {
            return false;
        }
        let entry = self.entries.remove(index);
        let destination = before
            .and_then(|target| self.entries.iter().position(|(key, _)| *key == target))
            .unwrap_or(self.entries.len());
        self.entries.insert(destination, entry);
        true
    }
    pub fn close(&mut self, id: TabId) -> Option<T> {
        let index = self.entries.iter().position(|(key, _)| *key == id)?;
        let (_, value) = self.entries.remove(index);
        if self.active == Some(id) {
            self.active = if self.entries.is_empty() {
                None
            } else {
                Some(self.entries[index.min(self.entries.len() - 1)].0)
            };
        }
        Some(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_preserves_selection_and_values_in_both_directions() {
        let mut tabs = Tabs::default();
        let a = add(&mut tabs, "a");
        let b = add(&mut tabs, "b");
        let c = add(&mut tabs, "c");
        tabs.select(b);
        assert!(tabs.move_before(c, Some(a)));
        assert_eq!(
            tabs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![c, a, b]
        );
        assert_eq!(tabs.active(), Some(&"b"));
        assert_eq!(tabs.get(c), Some(&"c"));
        assert!(tabs.move_before(c, None));
        assert_eq!(
            tabs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![a, b, c]
        );
        assert!(!tabs.move_before(a, Some(999)));
        assert!(!tabs.move_before(a, Some(a)));
        assert!(!tabs.move_before(999, None));
        tabs.close(b);
        assert_eq!(tabs.active_id(), Some(c));
    }
    fn add(tabs: &mut Tabs<&'static str>, label: &'static str) -> TabId {
        tabs.create(|_| Ok::<_, ()>(label)).unwrap()
    }
    #[test]
    fn switching_preserves_data_and_rejects_unknown_ids() {
        let mut tabs = Tabs::default();
        let a = add(&mut tabs, "a");
        let b = add(&mut tabs, "b");
        assert!(tabs.select(a));
        assert_eq!(tabs.active(), Some(&"a"));
        assert_eq!(tabs.get(b), Some(&"b"));
        assert!(!tabs.select(999));
        assert_eq!(tabs.active_id(), Some(a));
    }
    #[test]
    fn close_background_preserves_selection_and_close_active_selects_neighbor() {
        let mut tabs = Tabs::default();
        let a = add(&mut tabs, "a");
        let b = add(&mut tabs, "b");
        let c = add(&mut tabs, "c");
        tabs.select(b);
        assert_eq!(tabs.close(a), Some("a"));
        assert_eq!(tabs.active_id(), Some(b));
        assert_eq!(tabs.close(b), Some("b"));
        assert_eq!(tabs.active_id(), Some(c));
        assert!(tabs.get_mut(b).is_none());
        assert!(tabs.close(b).is_none());
    }
    #[test]
    fn ids_never_reused_and_failed_creation_preserves_active() {
        let mut tabs = Tabs::default();
        let a = add(&mut tabs, "a");
        tabs.close(a);
        assert!(tabs.is_empty());
        let b = add(&mut tabs, "b");
        assert!(b > a);
        assert!(tabs.create(|_| Err::<&str, _>("failed")).is_err());
        assert_eq!(tabs.active_id(), Some(b));
        let c = add(&mut tabs, "c");
        assert!(c > b + 1);
    }
}
