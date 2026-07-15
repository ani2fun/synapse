//! Domain → wire mapping (file-private to the http layer, oracle: `CatalogDtos`).

use synapse_shared::api::ApiError;
use synapse_shared::catalog::{
    BookDto, BookEntryDto, BookRefDto, CatalogEntryDto, CategoryDto, ChapterDto, ComponentDocDto, LessonDto,
    LessonFrontmatterDto, LessonPayloadDto, SynapseIndexDto,
};

use crate::catalog::application::ContentError;
use crate::catalog::domain::catalog::{BookEntry, CatalogEntry, Lesson, SynapseContentCatalog};
use crate::catalog::domain::component_doc::ComponentDoc;
use crate::catalog::domain::lesson::LessonContent;

pub fn to_index(catalog: &SynapseContentCatalog) -> SynapseIndexDto {
    SynapseIndexDto {
        entries: catalog.entries.iter().map(catalog_entry).collect(),
    }
}

fn catalog_entry(entry: &CatalogEntry) -> CatalogEntryDto {
    match entry {
        CatalogEntry::Category(c) => CatalogEntryDto::Category(CategoryDto {
            slug: c.slug.clone(),
            title: c.title.clone(),
            description: c.description.clone(),
            icon: c.icon.clone(),
            order: c.order,
            entries: c.entries.iter().map(catalog_entry).collect(),
        }),
        CatalogEntry::Book(b) => CatalogEntryDto::Book(BookDto {
            slug: b.slug.clone(),
            title: b.title.clone(),
            description: b.description.clone(),
            tags: b.tags.clone(),
            estimated_reading_minutes: b.estimated_reading_minutes,
            order: b.order,
            category_path: b.category_path.clone(),
            entries: b.entries.iter().map(book_entry).collect(),
        }),
    }
}

fn book_entry(entry: &BookEntry) -> BookEntryDto {
    match entry {
        BookEntry::Chapter {
            slug,
            title,
            order,
            entries,
        } => BookEntryDto::Chapter(ChapterDto {
            slug: slug.clone(),
            title: title.clone(),
            order: *order,
            entries: entries.iter().map(book_entry).collect(),
        }),
        BookEntry::Lesson(lesson) => BookEntryDto::Lesson(lesson_dto(lesson)),
    }
}

fn lesson_dto(lesson: &Lesson) -> LessonDto {
    LessonDto {
        slug: lesson.slug.clone(),
        title: lesson.title.clone(),
        order: lesson.order,
        essential: lesson.essential,
    }
}

/// Prev/next leave here as FULL directory-mirror paths: `category…/book slug/in-book path`.
pub fn to_payload(content: &LessonContent) -> LessonPayloadDto {
    let full = |in_book: &str| {
        let mut segments = content.book.category_path.clone();
        segments.push(content.book.slug.clone());
        format!("{}/{}", segments.join("/"), in_book)
    };
    LessonPayloadDto {
        book: BookRefDto {
            slug: content.book.slug.clone(),
            title: content.book.title.clone(),
            category_path: content.book.category_path.clone(),
        },
        lesson: lesson_dto(&content.lesson),
        frontmatter: LessonFrontmatterDto {
            title: content.frontmatter.title.clone(),
            summary: content.frontmatter.summary.clone(),
            essential: content.frontmatter.essential,
            kind: content.frontmatter.kind.clone(),
            difficulty: content.frontmatter.difficulty.clone(),
            topics: content.frontmatter.topics.clone(),
        },
        raw: content.raw.clone(),
        prev: content.prev_path.as_deref().map(full),
        next: content.next_path.as_deref().map(full),
        editorial: content.editorial.clone(),
    }
}

pub fn to_component_doc(doc: &ComponentDoc) -> ComponentDocDto {
    ComponentDocDto {
        title: doc.title.clone(),
        kind: doc.kind.clone(),
        technology: doc.technology.clone(),
        body: doc.body.clone(),
    }
}

/// `NotFound`→404 · `Io`→500 · `IndexInvalid`→500, always the `ApiError` envelope.
pub fn to_error(error: &ContentError) -> (axum::http::StatusCode, ApiError) {
    use axum::http::StatusCode;
    match error {
        ContentError::NotFound(detail) => (
            StatusCode::NOT_FOUND,
            ApiError {
                error: "Not found".to_owned(),
                detail: Some(detail.clone()),
                hint: None,
            },
        ),
        ContentError::Io(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiError {
                error: "Catalog IO error".to_owned(),
                detail: Some(detail.clone()),
                hint: None,
            },
        ),
        ContentError::IndexInvalid(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiError {
                error: "Catalog index invalid".to_owned(),
                detail: Some(err.to_string()),
                hint: None,
            },
        ),
    }
}
