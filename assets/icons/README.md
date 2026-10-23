# Row icons

The PNGs are Amazon Q Developer CLI's autocomplete icons, from
[aws/amazon-q-developer-cli-autocomplete](https://github.com/aws/amazon-q-developer-cli-autocomplete) (Apache-2.0):

- `crates/fig_desktop/icons/autocomplete/*.png`: `alert`, `asterisk`, `box`, `carrot`, `characters`, `command`,
  `commandkey`, `cpu`, `database`, `flag`, `gear`, `invite`, `option`, `package`, `string`, `symlink`, `template`.
  `file` and `folder` are the same files resized to 32×32 with `sips` (Q's build resizes with CatmullRom).
- The other names in `packages/autocomplete/src/fig/icons.ts` (`git`, `docker`, `npm`, `github`, …), downloaded from
  `https://specs.q.us-east-1.amazonaws.com/icons/<name>.png`, the CDN Q uses at runtime.

`template.rgba` is `template.png` decoded to 32×32 RGBA8, so the tint can be applied the way Q applies it.

The brand logos (`git`, `docker`, `aws`, …) are trademarks of their owners; they are included only to match Q's
autocomplete.
