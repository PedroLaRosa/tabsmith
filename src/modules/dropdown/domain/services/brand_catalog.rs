//! The curated brand table: which icon in `ICON_NAMES` stands for a command.

/// Command → icon name. Curated by hand: a command and a logo sharing a name is no proof they are the same product.
const BRANDS: &[(&str, &str)] = &[
    ("adb", "android"),
    ("ansible", "ansible"),
    ("argo", "argo"),
    ("asciinema", "asciinema"),
    ("astro", "astro"),
    ("autocannon", "autocannon"),
    ("aws", "aws"),
    ("az", "azure"),
    ("babel", "babel"),
    ("bat", "bat"),
    ("bazel", "bazel"),
    ("bit", "bit"),
    ("black", "black"),
    ("brew", "homebrew"),
    ("bun", "bun"),
    ("bunx", "bun"),
    ("capacitor", "capacitor"),
    ("circleci", "circleci"),
    ("claude", "claude"),
    ("clojure", "clojure"),
    ("cmake", "cmake"),
    ("coda", "coda"),
    ("curl", "curl"),
    ("dapr", "dapr"),
    ("dart", "dart"),
    ("docker", "docker"),
    ("docker-compose", "docker"),
    ("dotnet", "dotnet"),
    ("electron", "electron"),
    ("eleventy", "eleventy"),
    ("elm", "elm"),
    ("esbuild", "esbuild"),
    ("eslint", "eslint"),
    ("expo", "expo"),
    ("fastlane", "fastlane"),
    ("fastly", "fastly"),
    ("ffmpeg", "ffmpeg"),
    ("fig", "fig"),
    ("firebase", "firebase"),
    ("gatsby", "gatsby"),
    ("gcloud", "gcloud"),
    ("gh", "github"),
    ("git", "git"),
    ("github", "github"),
    ("glab", "gitlab"),
    ("gradle", "gradle"),
    ("hasura", "hasura"),
    ("helm", "helm"),
    ("heroku", "heroku"),
    ("hexo", "hexo"),
    ("htop", "htop"),
    ("hugo", "hugo"),
    ("hyper", "hyper"),
    ("infracost", "infracost"),
    ("ionic", "ionic"),
    ("jest", "jest"),
    ("julia", "julia"),
    ("jupyter", "jupyter"),
    ("just", "just"),
    ("k6", "k6"),
    ("kubecolor", "kubernetes"),
    ("kubectl", "kubernetes"),
    ("kubectx", "kubernetes"),
    ("kubens", "kubernetes"),
    ("laravel", "laravel"),
    ("lerna", "lerna"),
    ("locust", "locust"),
    ("meteor", "meteor"),
    ("nativescript", "nativescript"),
    ("netlify", "netlify"),
    ("nextflow", "nextflow"),
    ("ngrok", "ngrok"),
    ("nhost", "nhost"),
    ("node", "nodedotjs"),
    ("npm", "npm"),
    ("npx", "npm"),
    ("nuxi", "nuxt"),
    ("nuxt", "nuxt"),
    ("nvm", "nvm"),
    ("nx", "nx"),
    ("okteto", "okteto"),
    ("ollama", "ollama"),
    ("opencode", "opencode"),
    ("pi", "pi"),
    ("pipx", "pipx"),
    ("pm2", "pm2"),
    ("pnpm", "pnpm"),
    ("pnpx", "pnpm"),
    ("pocketbase", "pocketbase"),
    ("podman", "podman"),
    ("poetry", "poetry"),
    ("prettier", "prettier"),
    ("prisma", "prisma"),
    ("pytest", "pytest"),
    ("railway", "railway"),
    ("rancher", "rancher"),
    ("rclone", "rclone"),
    ("remix", "remix"),
    ("rich", "rich"),
    ("ruff", "ruff"),
    ("sequelize", "sequelize"),
    ("serverless", "serverless"),
    ("simctl", "apple"),
    ("speedtest", "speedtest"),
    ("spring", "spring"),
    ("stencil", "stencil"),
    ("streamlit", "streamlit"),
    ("stripe", "stripe"),
    ("supabase", "supabase"),
    ("swc", "swc"),
    ("tailscale", "tailscale"),
    ("tailwindcss", "tailwindcss"),
    ("tmux", "tmux"),
    ("tofu", "opentofu"),
    ("tsc", "typescript"),
    ("typst", "typst"),
    ("uv", "uv"),
    ("vapor", "vapor"),
    ("vercel", "vercel"),
    ("vim", "vim"),
    ("vite", "vite"),
    ("vtex", "vtex"),
    ("wezterm", "wezterm"),
    ("xcode-select", "apple"),
    ("xcodebuild", "apple"),
    ("xcrun", "apple"),
    ("yarn", "yarn"),
    ("zsh", "zsh"),
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
    fn should_give_node_the_nodejs_logo_not_the_graph_node_icon_named_node() {
        let brand = brand_icon("node");

        assert_eq!(brand, Some("nodedotjs"));
    }

    #[test]
    fn should_give_pnpm_its_brand() {
        let brand = brand_icon("pnpm");

        assert_eq!(brand, Some("pnpm"));
    }

    #[test]
    fn should_map_the_commands_of_the_brands_drawn_from_simple_icons() {
        let expected = [
            ("ansible", "ansible"),
            ("argo", "argo"),
            ("asciinema", "asciinema"),
            ("astro", "astro"),
            ("autocannon", "autocannon"),
            ("babel", "babel"),
            ("bat", "bat"),
            ("bazel", "bazel"),
            ("bit", "bit"),
            ("black", "black"),
            ("bun", "bun"),
            ("capacitor", "capacitor"),
            ("circleci", "circleci"),
            ("claude", "claude"),
            ("clojure", "clojure"),
            ("cmake", "cmake"),
            ("coda", "coda"),
            ("curl", "curl"),
            ("dapr", "dapr"),
            ("dart", "dart"),
            ("dotnet", "dotnet"),
            ("electron", "electron"),
            ("eleventy", "eleventy"),
            ("elm", "elm"),
            ("esbuild", "esbuild"),
            ("eslint", "eslint"),
            ("expo", "expo"),
            ("fastlane", "fastlane"),
            ("fastly", "fastly"),
            ("ffmpeg", "ffmpeg"),
            ("fig", "fig"),
            ("gatsby", "gatsby"),
            ("hasura", "hasura"),
            ("helm", "helm"),
            ("hexo", "hexo"),
            ("htop", "htop"),
            ("hugo", "hugo"),
            ("hyper", "hyper"),
            ("infracost", "infracost"),
            ("ionic", "ionic"),
            ("jest", "jest"),
            ("julia", "julia"),
            ("jupyter", "jupyter"),
            ("just", "just"),
            ("k6", "k6"),
            ("laravel", "laravel"),
            ("lerna", "lerna"),
            ("locust", "locust"),
            ("meteor", "meteor"),
            ("nativescript", "nativescript"),
            ("nextflow", "nextflow"),
            ("ngrok", "ngrok"),
            ("nhost", "nhost"),
            ("nuxt", "nuxt"),
            ("nvm", "nvm"),
            ("nx", "nx"),
            ("ollama", "ollama"),
            ("opencode", "opencode"),
            ("pi", "pi"),
            ("pipx", "pipx"),
            ("pm2", "pm2"),
            ("pocketbase", "pocketbase"),
            ("podman", "podman"),
            ("poetry", "poetry"),
            ("prettier", "prettier"),
            ("prisma", "prisma"),
            ("pytest", "pytest"),
            ("railway", "railway"),
            ("rancher", "rancher"),
            ("rclone", "rclone"),
            ("remix", "remix"),
            ("rich", "rich"),
            ("ruff", "ruff"),
            ("sequelize", "sequelize"),
            ("serverless", "serverless"),
            ("speedtest", "speedtest"),
            ("spring", "spring"),
            ("stencil", "stencil"),
            ("streamlit", "streamlit"),
            ("stripe", "stripe"),
            ("supabase", "supabase"),
            ("swc", "swc"),
            ("tailscale", "tailscale"),
            ("tailwindcss", "tailwindcss"),
            ("tmux", "tmux"),
            ("typst", "typst"),
            ("uv", "uv"),
            ("vapor", "vapor"),
            ("vim", "vim"),
            ("vite", "vite"),
            ("vtex", "vtex"),
            ("wezterm", "wezterm"),
            ("zsh", "zsh"),
            ("pnpx", "pnpm"),
            ("bunx", "bun"),
            ("brew", "homebrew"),
            ("tofu", "opentofu"),
            ("tsc", "typescript"),
            ("nuxi", "nuxt"),
            ("github", "github"),
            ("kubecolor", "kubernetes"),
            ("xcode-select", "apple"),
            ("simctl", "apple"),
        ];

        let missing: Vec<_> = expected
            .iter()
            .filter(|(command, icon)| brand_icon(command) != Some(icon))
            .collect();

        assert_eq!(missing, Vec::<&(&str, &str)>::new());
    }

    #[test]
    fn should_give_no_brand_to_commands_named_like_another_product() {
        let namesakes = ["less", "man", "make", "nano", "hermes", "task", "dotenv"];

        let branded: Vec<_> = namesakes.into_iter().filter(|command| brand_icon(command).is_some()).collect();

        assert_eq!(branded, Vec::<&str>::new());
    }

    #[test]
    fn should_give_no_brand_to_commands_whose_logo_licence_does_not_allow_it() {
        let restricted = [
            "cargo", "rustc", "rustup", "nvim", "fish", "kitty", "rubocop", "webpack", "quasar", "php", "zig", "r",
            "pandoc", "sidekiq", "ruby", "pod", "mvn", "deno", "exercism", "bash", "java", "javac", "sanity", "typeorm",
            "v",
        ];

        let branded: Vec<_> = restricted.into_iter().filter(|command| brand_icon(command).is_some()).collect();

        assert_eq!(branded, Vec::<&str>::new());
    }

    #[test]
    fn should_give_no_brand_to_commands_whose_brand_guidelines_do_not_allow_it() {
        let restricted = [
            "appwrite", "wrangler", "elixir", "mix", "iex", "firefox", "flutter", "fly", "flyctl", "go", "mysql", "nginx",
            "okta", "psql", "pg_dump", "createdb", "pulumi", "python", "python3", "pip", "pip3", "clion", "goland",
            "phpstorm", "pycharm", "rubymine", "webstorm", "shopify", "spotify", "swift", "symfony", "terraform", "vault",
            "trivy", "turbo", "zapier",
        ];

        let branded: Vec<_> = restricted.into_iter().filter(|command| brand_icon(command).is_some()).collect();

        assert_eq!(branded, Vec::<&str>::new());
    }

    #[test]
    fn should_give_no_brand_to_a_tool_whose_logo_is_unreadable_at_32_pixels() {
        let brand = brand_icon("composer");

        assert_eq!(brand, None);
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
