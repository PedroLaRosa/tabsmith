#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum KeyAction {
    InsertSelected,
    InsertCommonPrefix,
    InsertCommonPrefixOrNavigateDown,
    InsertCommonPrefixOrInsertSelected,
    InsertSelectedAndExecute,
    Execute,
    NavigateUp,
    NavigateDown,
    HideAutocomplete,
    ShowAutocomplete,
    ToggleAutocomplete,
    ToggleHistoryMode,
    ToggleDescription,
    ShowDescription,
    HideDescription,
    ToggleFuzzySearch,
    /// 1..=10
    SelectSuggestion(u8),
}

impl KeyAction {
    /// Action id (`insertSelected`, `selectSuggestion3`, …); an `autocomplete.` prefix is accepted.
    pub(crate) fn from_id(id: &str) -> Option<KeyAction> {
        use KeyAction::*;
        let id = id.strip_prefix("autocomplete.").unwrap_or(id);
        Some(match id {
            "insertSelected" => InsertSelected,
            "insertCommonPrefix" => InsertCommonPrefix,
            "insertCommonPrefixOrNavigateDown" => InsertCommonPrefixOrNavigateDown,
            "insertCommonPrefixOrInsertSelected" => InsertCommonPrefixOrInsertSelected,
            "insertSelectedAndExecute" => InsertSelectedAndExecute,
            "execute" => Execute,
            "navigateUp" => NavigateUp,
            "navigateDown" => NavigateDown,
            "hideAutocomplete" => HideAutocomplete,
            "showAutocomplete" => ShowAutocomplete,
            "toggleAutocomplete" => ToggleAutocomplete,
            "toggleHistoryMode" => ToggleHistoryMode,
            "toggleDescription" => ToggleDescription,
            "showDescription" => ShowDescription,
            "hideDescription" => HideDescription,
            "toggleFuzzySearch" => ToggleFuzzySearch,
            _ => {
                let n: u8 = id.strip_prefix("selectSuggestion")?.parse().ok()?;
                return (1..=10).contains(&n).then_some(SelectSuggestion(n));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids() {
        assert_eq!(
            KeyAction::from_id("insertSelected"),
            Some(KeyAction::InsertSelected)
        );
        assert_eq!(
            KeyAction::from_id("autocomplete.navigateDown"),
            Some(KeyAction::NavigateDown)
        );
        assert_eq!(
            KeyAction::from_id("selectSuggestion10"),
            Some(KeyAction::SelectSuggestion(10))
        );
        assert_eq!(KeyAction::from_id("selectSuggestion0"), None);
        assert_eq!(KeyAction::from_id("increaseSize"), None);
    }

    #[test]
    fn should_read_the_select_suggestion_number_as_a_u8() {
        let ids = [
            "selectSuggestion+5",
            "selectSuggestion05",
            "autocomplete.selectSuggestion3",
            "selectSuggestion10",
            "selectSuggestion11",
            "selectSuggestion256",
            "selectSuggestion-1",
            "selectSuggestion",
            "autocomplete.autocomplete.insertSelected",
        ]
        .map(KeyAction::from_id);

        assert_eq!(
            ids,
            [
                Some(KeyAction::SelectSuggestion(5)),
                Some(KeyAction::SelectSuggestion(5)),
                Some(KeyAction::SelectSuggestion(3)),
                Some(KeyAction::SelectSuggestion(10)),
                None,
                None,
                None,
                None,
                None,
            ]
        );
    }
}
