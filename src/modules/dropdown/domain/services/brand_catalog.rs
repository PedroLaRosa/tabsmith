//! The curated brand table: which icon in `ICON_NAMES` stands for a command.

/// Command → icon name. Curated by hand: a command and a logo sharing a name is no proof they are the same product.
const BRANDS: &[(&str, &str)] = &[
    ("git", "git"),
    ("kubectl", "kubernetes"),
    ("kubectx", "kubernetes"),
    ("kubens", "kubernetes"),
];

/// The icon of `command`'s brand, if the table has one.
pub(crate) fn brand_icon(command: &str) -> Option<&'static str> {
    BRANDS
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, icon)| *icon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_name_the_git_icon_as_the_brand_of_git() {
        let brand = brand_icon("git");

        assert_eq!(brand, Some("git"));
    }

    #[test]
    fn should_give_several_commands_of_one_product_the_same_brand() {
        let commands = ["kubectl", "kubectx", "kubens"];

        let brands = commands.map(brand_icon);

        assert_eq!(brands, [Some("kubernetes"); 3]);
    }
}
