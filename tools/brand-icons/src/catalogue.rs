fn is_allowed(_licence: Option<&str>) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_allow_an_entry_without_a_licence_of_its_own() {
        let licence = None;

        let allowed = is_allowed(licence);

        assert!(allowed);
    }
}
