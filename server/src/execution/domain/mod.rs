//! Pure execution domain — the language model (oracle: `Language.scala`). No sandbox ids, no
//! magic ints: languages are an enum with labels and fence aliases (the code-quality bar's
//! canonical "model it as an enum" example).

mod language;

pub use language::Language;
