use std::collections::HashMap;

use annotate_snippets::{Annotation, AnnotationType, Renderer, Slice, Snippet, SourceAnnotation};

use stable_mir::ty::Span;

pub fn create_error(spans: &[(Span, Option<&str>)], message: impl std::fmt::Display) {
    let mut sources = HashMap::new();
    let filenames: Vec<_> = spans.iter().map(|(span, _)| span.get_filename()).collect();
    for filename in &filenames {
        sources.entry(filename.clone()).or_insert_with(|| {
            let source = std::fs::read_to_string(filename).unwrap();
            let lines = source
                .char_indices()
                .filter(|&(_, c)| c == '\n')
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            (source, lines)
        });
    }
    let message = message.to_string();
    let mut slices = vec![];
    for ((span, label), filename) in spans.into_iter().zip(&filenames) {
        let (source, lines) = sources.get(filename).unwrap();

        let stable_mir::ty::LineInfo {
            start_line,
            end_line,
            start_col,
            end_col,
        } = span.get_lines();

        let source = &source[lines[start_line.saturating_sub(2)]..=lines[end_line]];
        let len = source.chars().count();
        let range = if end_line > start_line {
            (start_col, len)
        } else if start_col == end_col {
            if start_col - 1 == len {
                // rustc sometimes produces spans pointing *after* the `\n` at the end of the line,
                // but we want to render an annotation at the end.
                (start_col - 1, start_col)
            } else {
                (start_col, start_col + 1)
            }
        } else {
            (start_col, end_col)
        };
        slices.push(Slice {
            source,
            line_start: start_line,
            origin: Some(&filename),
            annotations: label
                .as_ref()
                .into_iter()
                .map(|label| SourceAnnotation {
                    range,
                    label,
                    annotation_type: AnnotationType::Error,
                })
                .collect(),
            fold: false,
        });
    }
    let msg = Snippet {
        title: Some(Annotation {
            id: None,
            annotation_type: AnnotationType::Error,
            label: Some(&message),
        }),
        slices,
        footer: vec![],
    };
    let renderer = Renderer::styled();
    println!("{}", renderer.render(msg));
}
