use ariadne::{Color, Label, Report, ReportKind, Source};
use rustpython_parser::ParseError;
use crate::lowering::LoweringError;
use crate::codegen::CodeGenError;

/// Display a parse error with ariadne formatting
pub fn display_parse_error(source: &str, filename: &str, error: &ParseError) {
    let offset = usize::from(error.offset);
    let mut line = 1;
    let mut column = 1;

    for (i, ch) in source.chars().enumerate() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    // Calculate end offset (just one character after start for simplicity)
    let end_offset = std::cmp::min(offset + 1, source.len());

    Report::build(ReportKind::Error, filename, offset)
        .with_message(format!("Parse error: {}", error.error))
        .with_label(
            Label::new((filename, offset..end_offset))
                .with_message(format!("{}:{}: {}", line, column, error.error))
                .with_color(Color::Red),
        )
        .finish()
        .eprint((filename, Source::from(source)))
        .unwrap();
}

/// Display a lowering error with ariadne formatting
pub fn display_lowering_error(source: &str, filename: &str, error: &LoweringError) {
    Report::build(ReportKind::Error, filename, 0)
        .with_message("Lowering error")
        .with_label(
            Label::new((filename, 0..1))
                .with_message(error.to_string())
                .with_color(Color::Red),
        )
        .finish()
        .eprint((filename, Source::from(source)))
        .unwrap();
}

/// Display a code generation error with ariadne formatting
pub fn display_codegen_error(source: &str, filename: &str, error: &CodeGenError) {
    Report::build(ReportKind::Error, filename, 0)
        .with_message("Code generation error")
        .with_label(
            Label::new((filename, 0..1))
                .with_message(error.to_string())
                .with_color(Color::Red),
        )
        .finish()
        .eprint((filename, Source::from(source)))
        .unwrap();
}
