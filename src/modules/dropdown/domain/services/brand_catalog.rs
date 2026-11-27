//! The curated brand table: which icon in `ICON_NAMES` stands for a command.

/// Command → icon name. Curated by hand: a command and a logo sharing a name is no proof they are the same product.
const BRANDS: &[(&str, &str)] = &[
    ("adb", "android"),
    ("aws", "aws"),
    ("az", "azure"),
    ("docker", "docker"),
    ("docker-compose", "docker"),
    ("firebase", "firebase"),
    ("gcloud", "gcloud"),
    ("gh", "github"),
    ("git", "git"),
    ("glab", "gitlab"),
    ("gradle", "gradle"),
    ("heroku", "heroku"),
    ("kubectl", "kubernetes"),
    ("kubectx", "kubernetes"),
    ("kubens", "kubernetes"),
    ("netlify", "netlify"),
    ("node", "node"),
    ("npm", "npm"),
    ("npx", "npm"),
    ("okteto", "okteto"),
    ("vercel", "vercel"),
    ("xcodebuild", "apple"),
    ("xcrun", "apple"),
    ("yarn", "yarn"),
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
    use crate::modules::dropdown::domain::value_objects::icon_id::ICON_NAMES;

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

    #[test]
    fn should_map_the_commands_of_the_brands_that_already_have_an_icon() {
        let expected = [
            ("docker", "docker"),
            ("docker-compose", "docker"),
            ("npm", "npm"),
            ("npx", "npm"),
            ("yarn", "yarn"),
            ("node", "node"),
            ("gcloud", "gcloud"),
            ("aws", "aws"),
            ("az", "azure"),
            ("gh", "github"),
            ("glab", "gitlab"),
            ("gradle", "gradle"),
            ("firebase", "firebase"),
            ("heroku", "heroku"),
            ("netlify", "netlify"),
            ("vercel", "vercel"),
            ("adb", "android"),
            ("okteto", "okteto"),
            ("xcrun", "apple"),
            ("xcodebuild", "apple"),
        ];

        let missing: Vec<_> = expected
            .iter()
            .filter(|(command, icon)| brand_icon(command) != Some(icon))
            .collect();

        assert_eq!(missing, Vec::<&(&str, &str)>::new());
    }

    #[test]
    fn should_name_only_icons_that_exist() {
        let unknown: Vec<_> = BRANDS
            .iter()
            .filter(|(_, icon)| !ICON_NAMES.contains(icon))
            .collect();

        assert_eq!(unknown, Vec::<&(&str, &str)>::new());
    }
}
