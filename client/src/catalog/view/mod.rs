//! The catalog views (the view layer): the library (browse) and the reader (lesson + sidebar).
//! Markdown crosses the island bridge; everything else is signals → DOM.

mod library;
mod reader;

pub use library::LibraryPage;
pub use reader::LessonPage;
