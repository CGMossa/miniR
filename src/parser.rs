pub mod ast;
mod builder;
mod diagnostics;
mod literals;

use pest::Parser;
use pest_derive::Parser;

use builder::build_program;
use diagnostics::convert_pest_error;
pub use diagnostics::ParseError;

#[derive(Parser)]
#[grammar = "parser/r.pest"]
pub struct RParser;

pub fn parse_program(input: &str) -> Result<ast::Expr, ParseError> {
    let pairs = RParser::parse(Rule::program, input).map_err(|e| convert_pest_error(e, input))?;

    let pair = pairs.into_iter().next().unwrap();
    Ok(build_program(pair))
}
