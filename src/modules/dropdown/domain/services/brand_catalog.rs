//! The curated brand table: which icon in `ICON_NAMES` stands for a command.

/// Command → icon name. Curated by hand: a command and a logo sharing a name is no proof they are the same product.
const BRANDS: &[(&str, &str)] = &[
    ("adb", "android"),
    ("ansible", "ansible"),
    ("appwrite", "appwrite"),
    ("argo", "argo"),
    ("asciinema", "asciinema"),
    ("astro", "astro"),
    ("autocannon", "autocannon"),
    ("aws", "aws"),
    ("az", "azure"),
    ("babel", "babel"),
    ("bash", "gnubash"),
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
    ("clion", "clion"),
    ("clojure", "clojure"),
    ("cmake", "cmake"),
    ("coda", "coda"),
    ("composer", "composer"),
    ("createdb", "postgresql"),
    ("curl", "curl"),
    ("dapr", "dapr"),
    ("dart", "dart"),
    ("deno", "deno"),
    ("docker", "docker"),
    ("docker-compose", "docker"),
    ("dotenv", "dotenv"),
    ("dotnet", "dotnet"),
    ("electron", "electron"),
    ("eleventy", "eleventy"),
    ("elixir", "elixir"),
    ("elm", "elm"),
    ("esbuild", "esbuild"),
    ("eslint", "eslint"),
    ("exercism", "exercism"),
    ("expo", "expo"),
    ("fastlane", "fastlane"),
    ("fastly", "fastly"),
    ("ffmpeg", "ffmpeg"),
    ("fig", "fig"),
    ("firebase", "firebase"),
    ("firefox", "firefox"),
    ("flutter", "flutter"),
    ("fly", "flydotio"),
    ("flyctl", "flydotio"),
    ("gatsby", "gatsby"),
    ("gcloud", "gcloud"),
    ("gh", "github"),
    ("git", "git"),
    ("github", "github"),
    ("glab", "gitlab"),
    ("go", "go"),
    ("goland", "goland"),
    ("gradle", "gradle"),
    ("hasura", "hasura"),
    ("helm", "helm"),
    ("heroku", "heroku"),
    ("hexo", "hexo"),
    ("htop", "htop"),
    ("hugo", "hugo"),
    ("hyper", "hyper"),
    ("iex", "elixir"),
    ("infracost", "infracost"),
    ("ionic", "ionic"),
    ("java", "openjdk"),
    ("javac", "openjdk"),
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
    ("mix", "elixir"),
    ("mvn", "apachemaven"),
    ("mysql", "mysql"),
    ("nativescript", "nativescript"),
    ("netlify", "netlify"),
    ("nextflow", "nextflow"),
    ("nginx", "nginx"),
    ("ngrok", "ngrok"),
    ("nhost", "nhost"),
    ("node", "nodedotjs"),
    ("npm", "npm"),
    ("npx", "npm"),
    ("nuxi", "nuxt"),
    ("nuxt", "nuxt"),
    ("nvm", "nvm"),
    ("nx", "nx"),
    ("okta", "okta"),
    ("okteto", "okteto"),
    ("ollama", "ollama"),
    ("opencode", "opencode"),
    ("pg_dump", "postgresql"),
    ("phpstorm", "phpstorm"),
    ("pi", "pi"),
    ("pip", "python"),
    ("pip3", "python"),
    ("pipx", "pipx"),
    ("pm2", "pm2"),
    ("pnpm", "pnpm"),
    ("pnpx", "pnpm"),
    ("pocketbase", "pocketbase"),
    ("podman", "podman"),
    ("poetry", "poetry"),
    ("prettier", "prettier"),
    ("prisma", "prisma"),
    ("psql", "postgresql"),
    ("pulumi", "pulumi"),
    ("pycharm", "pycharm"),
    ("pytest", "pytest"),
    ("python", "python"),
    ("python3", "python"),
    ("railway", "railway"),
    ("rancher", "rancher"),
    ("rclone", "rclone"),
    ("remix", "remix"),
    ("rich", "rich"),
    ("rubymine", "rubymine"),
    ("ruff", "ruff"),
    ("sanity", "sanity"),
    ("sequelize", "sequelize"),
    ("serverless", "serverless"),
    ("shopify", "shopify"),
    ("simctl", "apple"),
    ("speedtest", "speedtest"),
    ("spotify", "spotify"),
    ("spring", "spring"),
    ("stencil", "stencil"),
    ("streamlit", "streamlit"),
    ("stripe", "stripe"),
    ("supabase", "supabase"),
    ("swc", "swc"),
    ("swift", "swift"),
    ("symfony", "symfony"),
    ("tailscale", "tailscale"),
    ("tailwindcss", "tailwindcss"),
    ("terraform", "terraform"),
    ("tmux", "tmux"),
    ("tofu", "opentofu"),
    ("trivy", "trivy"),
    ("tsc", "typescript"),
    ("turbo", "turborepo"),
    ("typeorm", "typeorm"),
    ("typst", "typst"),
    ("uv", "uv"),
    ("v", "v"),
    ("vapor", "vapor"),
    ("vault", "vault"),
    ("vercel", "vercel"),
    ("vim", "vim"),
    ("vite", "vite"),
    ("vtex", "vtex"),
    ("webstorm", "webstorm"),
    ("wezterm", "wezterm"),
    ("wrangler", "cloudflare"),
    ("xcode-select", "apple"),
    ("xcodebuild", "apple"),
    ("xcrun", "apple"),
    ("yarn", "yarn"),
    ("zapier", "zapier"),
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
    fn should_give_python3_and_pnpm_their_brands() {
        let commands = ["python3", "pnpm"];

        let brands = commands.map(brand_icon);

        assert_eq!(brands, [Some("python"), Some("pnpm")]);
    }

    #[test]
    fn should_map_the_commands_of_the_brands_drawn_from_simple_icons() {
        let expected = [
            ("ansible", "ansible"),
            ("appwrite", "appwrite"),
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
            ("clion", "clion"),
            ("clojure", "clojure"),
            ("cmake", "cmake"),
            ("coda", "coda"),
            ("composer", "composer"),
            ("curl", "curl"),
            ("dapr", "dapr"),
            ("dart", "dart"),
            ("deno", "deno"),
            ("dotenv", "dotenv"),
            ("dotnet", "dotnet"),
            ("electron", "electron"),
            ("eleventy", "eleventy"),
            ("elixir", "elixir"),
            ("elm", "elm"),
            ("esbuild", "esbuild"),
            ("eslint", "eslint"),
            ("exercism", "exercism"),
            ("expo", "expo"),
            ("fastlane", "fastlane"),
            ("fastly", "fastly"),
            ("ffmpeg", "ffmpeg"),
            ("fig", "fig"),
            ("firefox", "firefox"),
            ("flutter", "flutter"),
            ("gatsby", "gatsby"),
            ("go", "go"),
            ("goland", "goland"),
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
            ("mysql", "mysql"),
            ("nativescript", "nativescript"),
            ("nextflow", "nextflow"),
            ("nginx", "nginx"),
            ("ngrok", "ngrok"),
            ("nhost", "nhost"),
            ("nuxt", "nuxt"),
            ("nvm", "nvm"),
            ("nx", "nx"),
            ("okta", "okta"),
            ("ollama", "ollama"),
            ("opencode", "opencode"),
            ("phpstorm", "phpstorm"),
            ("pi", "pi"),
            ("pipx", "pipx"),
            ("pm2", "pm2"),
            ("pocketbase", "pocketbase"),
            ("podman", "podman"),
            ("poetry", "poetry"),
            ("prettier", "prettier"),
            ("prisma", "prisma"),
            ("pulumi", "pulumi"),
            ("pycharm", "pycharm"),
            ("pytest", "pytest"),
            ("python", "python"),
            ("railway", "railway"),
            ("rancher", "rancher"),
            ("rclone", "rclone"),
            ("remix", "remix"),
            ("rich", "rich"),
            ("rubymine", "rubymine"),
            ("ruff", "ruff"),
            ("sanity", "sanity"),
            ("sequelize", "sequelize"),
            ("serverless", "serverless"),
            ("shopify", "shopify"),
            ("speedtest", "speedtest"),
            ("spotify", "spotify"),
            ("spring", "spring"),
            ("stencil", "stencil"),
            ("streamlit", "streamlit"),
            ("stripe", "stripe"),
            ("supabase", "supabase"),
            ("swc", "swc"),
            ("swift", "swift"),
            ("symfony", "symfony"),
            ("tailscale", "tailscale"),
            ("tailwindcss", "tailwindcss"),
            ("terraform", "terraform"),
            ("tmux", "tmux"),
            ("trivy", "trivy"),
            ("typeorm", "typeorm"),
            ("typst", "typst"),
            ("uv", "uv"),
            ("v", "v"),
            ("vapor", "vapor"),
            ("vault", "vault"),
            ("vim", "vim"),
            ("vite", "vite"),
            ("vtex", "vtex"),
            ("webstorm", "webstorm"),
            ("wezterm", "wezterm"),
            ("zapier", "zapier"),
            ("zsh", "zsh"),
            ("pip", "python"),
            ("pip3", "python"),
            ("pnpx", "pnpm"),
            ("bunx", "bun"),
            ("brew", "homebrew"),
            ("psql", "postgresql"),
            ("pg_dump", "postgresql"),
            ("createdb", "postgresql"),
            ("mvn", "apachemaven"),
            ("java", "openjdk"),
            ("javac", "openjdk"),
            ("tofu", "opentofu"),
            ("wrangler", "cloudflare"),
            ("fly", "flydotio"),
            ("flyctl", "flydotio"),
            ("tsc", "typescript"),
            ("bash", "gnubash"),
            ("turbo", "turborepo"),
            ("mix", "elixir"),
            ("iex", "elixir"),
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
    fn should_name_only_icons_that_exist() {
        let unknown: Vec<_> = BRANDS
            .iter()
            .filter(|(_, icon)| !ICON_NAMES.contains(icon))
            .collect();

        assert_eq!(unknown, Vec::<&(&str, &str)>::new());
    }
}
