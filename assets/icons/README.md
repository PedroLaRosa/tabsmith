# Row icons

## Amazon Q icons

The 42 PNGs not listed in `simple-icons.txt` are the autocomplete icons of
[aws/amazon-q-developer-cli-autocomplete](https://github.com/aws/amazon-q-developer-cli-autocomplete), used under its
Apache-2.0 licence (<https://www.apache.org/licenses/LICENSE-2.0>).

Changes made to them: `file` and `folder` are resized to 32×32 with `sips`. `template.rgba` is `template.png` decoded
to 32×32 RGBA8, so the tint can be applied in code.

## Brand icons from simple-icons

The PNGs listed in `simple-icons.txt` are drawn by `just brand-icons` (`scripts/brand-icons.sh`, `tools/brand-icons`)
from [simple-icons](https://simple-icons.org) 16.38.1, whose icons are CC0-1.0
(<https://creativecommons.org/publicdomain/zero/1.0/>). Each one is `template.png` tinted with the brand's colour, as
tabsmith tints badges, with the simple-icons glyph centred on it in white, or in black on a light colour.

Only plain CC0 entries are drawn. The tool refuses an entry with a licence of its own: attribution licences (MIT,
Apache-2.0, BSD, CC-BY) ask for notices that tabsmith's binary does not ship, and share-alike, copyleft,
non-commercial, no-derivatives and custom terms do not allow this use. It also refuses an entry whose brand has
guidelines of its own until they have been read and the manifest line ends in `guidelines-ok`. These were read and
allow a row icon:

- `dotnet`: <https://github.com/dotnet/brand/blob/c7d0f51b8ec59531332d05fb27a5b758a7a3d689/dotnet-styleGuide-2024.pdf>
- `expo`: <https://expo.dev/brand/>
- `gatsby`: <https://www.gatsbyjs.com/guidelines/logo>
- `jupyter`: <https://github.com/jupyter/design>
- `nodedotjs`: <https://nodejs.org/en/about/branding>
- `nuxt`: <https://nuxt.com/design-kit>
- `rancher`: <https://rancher.com/brand-guidelines/>
- `streamlit`: <https://www.streamlit.io/brand>
- `tailwindcss`: <https://tailwindcss.com/brand>
- `typescript`: <https://www.typescriptlang.org/branding>
- `typst`: <https://typst.app/legal/brand>

Brands whose guidelines ask for written permission, allow the name only in words, or forbid recolouring the mark or
placing it on a tile are left out (python, go, terraform, the JetBrains IDEs and others): their commands keep their
kind icons.

To add a brand: add `<slug> <colour>` (plus `guidelines-ok` once its guidelines are read) to `simple-icons.txt` and run
`just brand-icons`; then append the slug at the end of `ICON_NAMES` (`src/modules/dropdown/domain/value_objects/icon_id.rs`)
and of `PNGS` (`src/modules/terminal_session/infrastructure/adapters/icon_assets.rs`), add its colour to `PALETTE`
(`icon_palette.rs`), and map its commands in `BRANDS` (`brand_catalog.rs`). To drop one, delete its line and its PNG
and remove it from those four tables.

## Trademarks

The brand logos (`git`, `docker`, `aws`, `pnpm`, `typescript`, …) are trademarks of their owners. They are included
only to identify each tool on its row, not to suggest any endorsement. In particular: Node.js® is a registered
trademark and the Node.js logo is a trademark of the OpenJS Foundation; "Jupyter" and the Jupyter logos are trademarks
or registered trademarks of LF Charities; Rancher and the cow logo are registered trademarks or trademarks of Rancher
Labs, Inc. (now SUSE) in the United States and other countries; Tailwind CSS and the Tailwind logo are trademarks of
Tailwind Labs Inc.
