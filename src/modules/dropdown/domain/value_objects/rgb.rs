/// A colour as `(red, green, blue)`.
// ponytail: a tuple alias like `AliasTable` and `SpecFunctionRef`: the renderer, the pty and the tests write colours as `(r, g, b)`; make it a newtype if a colour ever gets behaviour of its own.
pub(crate) type Rgb = (u8, u8, u8);
