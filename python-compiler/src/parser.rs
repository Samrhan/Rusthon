use rustpython_parser::{Parse, ast, ParseError};

pub fn parse_program(source: &str) -> Result<ast::Suite, ParseError> {
    let suite = ast::Suite::parse(source, "<input>")?;
    Ok(suite)
}
