pub mod ast;
mod diagnostics;

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use ast::*;
use diagnostics::convert_pest_error;
pub use diagnostics::ParseError;

#[derive(Parser)]
#[grammar = "parser/r.pest"]
pub struct RParser;

pub fn parse_program(input: &str) -> Result<Expr, ParseError> {
    let pairs = RParser::parse(Rule::program, input).map_err(|e| convert_pest_error(e, input))?;

    let pair = pairs.into_iter().next().unwrap();
    Ok(build_program(pair))
}

fn build_program(pair: Pair<Rule>) -> Expr {
    let mut exprs = Vec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr_seq => {
                for child in p.into_inner() {
                    if child.as_rule() == Rule::expr {
                        exprs.push(build_expr(child));
                    }
                }
            }
            Rule::EOI => {}
            _ => {}
        }
    }
    if exprs.len() == 1 {
        exprs.into_iter().next().unwrap()
    } else {
        Expr::Program(exprs)
    }
}

fn build_expr(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::expr => build_expr(pair.into_inner().next().unwrap()),
        Rule::help_expr => build_help(pair),
        Rule::assign_eq_expr => build_assign_eq(pair),
        Rule::assign_left_expr => build_assign_left(pair),
        Rule::assign_right_expr => build_assign_right(pair),
        Rule::formula_expr => build_formula(pair),
        Rule::or_expr => build_binary_left(pair, |op| match op.as_str() {
            "||" => BinaryOp::OrScalar,
            "|" => BinaryOp::Or,
            _ => unreachable!(),
        }),
        Rule::and_expr => build_binary_left(pair, |op| match op.as_str() {
            "&&" => BinaryOp::AndScalar,
            "&" => BinaryOp::And,
            _ => unreachable!(),
        }),
        Rule::not_expr => build_not(pair),
        Rule::compare_expr => build_binary_left(pair, |op| match op.as_str() {
            "==" => BinaryOp::Eq,
            "!=" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            ">" => BinaryOp::Gt,
            "<=" => BinaryOp::Le,
            ">=" => BinaryOp::Ge,
            _ => unreachable!(),
        }),
        Rule::add_expr => build_binary_left(pair, |op| match op.as_str() {
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            _ => unreachable!(),
        }),
        Rule::mul_expr => build_binary_left(pair, |op| match op.as_str() {
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            _ => unreachable!(),
        }),
        Rule::special_pipe_expr => build_special_pipe(pair),
        Rule::colon_expr => build_colon(pair),
        Rule::unary_expr => build_unary(pair),
        Rule::power_expr => build_power(pair),
        Rule::postfix_expr => build_postfix_expr(pair),
        Rule::namespace_expr => build_namespace_expr(pair),
        Rule::primary_expr => build_primary(pair),
        Rule::keyword_constant => build_primary(pair),
        _ => build_primary(pair),
    }
}

// "?" help — convert to help("topic") call
fn build_help(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    if first.as_rule() == Rule::help_expr {
        // Unary: "?foo" → help("foo")
        let topic = extract_help_topic(&first);
        return Expr::Call {
            func: Box::new(Expr::Symbol("help".to_string())),
            args: vec![Arg {
                name: None,
                value: Some(Expr::String(topic)),
            }],
        };
    } else {
        // Binary: expr ~ "?" ~ expr — just evaluate the LHS
        let lhs = build_expr(first);
        // Ignore the RHS (help topic)
        if inner.next().is_some() {
            // just return lhs
        }
        lhs
    }
}

/// Extract the topic name from a help expression for `?foo`.
/// Walks down to find the innermost symbol or string.
fn extract_help_topic(pair: &Pair<Rule>) -> String {
    let text = pair.as_str().trim();
    // Strip leading ? if present
    let text = text.strip_prefix('?').unwrap_or(text).trim();
    // If it's a simple identifier or string, use it directly
    text.to_string()
}

// "=" assignment (right-associative)
fn build_assign_eq(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let lhs = build_expr(inner.next().unwrap());
    match inner.next() {
        None => lhs,
        Some(op_pair) => {
            assert!(op_pair.as_rule() == Rule::eq_assign_op);
            let rhs = build_expr(inner.next().unwrap());
            Expr::Assign {
                op: AssignOp::Equals,
                target: Box::new(lhs),
                value: Box::new(rhs),
            }
        }
    }
}

// "<-" "<<-" assignment (right-associative)
fn build_assign_left(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let lhs = build_expr(inner.next().unwrap());
    match inner.next() {
        None => lhs,
        Some(op_pair) => {
            let op = match op_pair.as_str() {
                "<-" => AssignOp::LeftAssign,
                "<<-" => AssignOp::SuperAssign,
                _ => unreachable!(),
            };
            let rhs = build_expr(inner.next().unwrap());
            Expr::Assign {
                op,
                target: Box::new(lhs),
                value: Box::new(rhs),
            }
        }
    }
}

// "->" "->>" assignment (right-associative, but target/value are swapped)
fn build_assign_right(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut result = build_expr(inner.next().unwrap());
    while let Some(op_pair) = inner.next() {
        let op = match op_pair.as_str() {
            "->" => AssignOp::RightAssign,
            "->>" => AssignOp::RightSuperAssign,
            _ => unreachable!(),
        };
        let target = build_expr(inner.next().unwrap());
        result = Expr::Assign {
            op,
            target: Box::new(target),
            value: Box::new(result),
        };
    }
    result
}

// "~" formula (unary or binary)
fn build_formula(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();

    if first.as_rule() == Rule::formula_expr {
        // Unary formula: "~" ~ formula_expr
        let rhs = build_expr(first);
        Expr::Formula {
            lhs: None,
            rhs: Some(Box::new(rhs)),
        }
    } else {
        // Binary: or_expr ~ ("~" ~ or_expr)?
        let lhs = build_expr(first);
        match inner.next() {
            None => lhs,
            Some(rhs_pair) => {
                let rhs = build_expr(rhs_pair);
                Expr::Formula {
                    lhs: Some(Box::new(lhs)),
                    rhs: Some(Box::new(rhs)),
                }
            }
        }
    }
}

fn build_binary_left(pair: Pair<Rule>, map_op: impl Fn(&Pair<Rule>) -> BinaryOp) -> Expr {
    let mut inner = pair.into_inner();
    let mut lhs = build_expr(inner.next().unwrap());
    while let Some(op_pair) = inner.next() {
        let op = map_op(&op_pair);
        let rhs = build_expr(inner.next().unwrap());
        lhs = Expr::BinaryOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
    lhs
}

fn build_not(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    if first.as_rule() == Rule::compare_expr {
        build_expr(first)
    } else {
        // "!" ~ not_expr
        let operand = build_expr(first);
        Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(operand),
        }
    }
}

// %...% and |>
fn build_special_pipe(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut lhs = build_expr(inner.next().unwrap());
    while let Some(op_pair) = inner.next() {
        let op = match op_pair.as_rule() {
            Rule::pipe_op => BinaryOp::Pipe,
            Rule::special_op => match op_pair.as_str() {
                "%in%" => BinaryOp::Special(SpecialOp::In),
                "%*%" => BinaryOp::Special(SpecialOp::MatMul),
                "%%" => BinaryOp::Mod,
                "%/%" => BinaryOp::IntDiv,
                _ => BinaryOp::Special(SpecialOp::Other),
            },
            _ => unreachable!(),
        };
        let rhs = build_expr(inner.next().unwrap());
        lhs = Expr::BinaryOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
    lhs
}

// ":" range/sequence (left-associative, chainable)
fn build_colon(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut lhs = build_expr(inner.next().unwrap());
    for rhs_pair in inner {
        lhs = Expr::BinaryOp {
            op: BinaryOp::Range,
            lhs: Box::new(lhs),
            rhs: Box::new(build_expr(rhs_pair)),
        };
    }
    lhs
}

fn build_unary(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    match first.as_rule() {
        Rule::unary_op => {
            let op = match first.as_str() {
                "-" => UnaryOp::Neg,
                "+" => UnaryOp::Pos,
                _ => unreachable!(),
            };
            let operand = build_expr(inner.next().unwrap());
            Expr::UnaryOp {
                op,
                operand: Box::new(operand),
            }
        }
        // "!" at unary level (allows a == !b)
        Rule::unary_expr => {
            let operand = build_expr(first);
            Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            }
        }
        _ => build_expr(first),
    }
}

fn build_power(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let base = build_expr(inner.next().unwrap());
    // Skip the power_op token if present
    match inner.next() {
        None => base,
        Some(next) => {
            let rhs_pair = if next.as_rule() == Rule::power_op {
                inner.next().unwrap()
            } else {
                next
            };
            Expr::BinaryOp {
                op: BinaryOp::Pow,
                lhs: Box::new(base),
                rhs: Box::new(build_expr(rhs_pair)),
            }
        }
    }
}

// postfix_expr = { namespace_expr ~ postfix_suffix* }
fn build_postfix_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(inner.next().unwrap());
    for suffix in inner {
        expr = build_postfix_suffix(expr, suffix);
    }
    expr
}

fn build_postfix_suffix(object: Expr, pair: Pair<Rule>) -> Expr {
    // Unwrap postfix_suffix wrapper if present
    let pair = if pair.as_rule() == Rule::postfix_suffix {
        pair.into_inner().next().unwrap()
    } else {
        pair
    };
    match pair.as_rule() {
        Rule::call_suffix => {
            let args = pair
                .into_inner()
                .filter(|p| p.as_rule() == Rule::arg_list)
                .flat_map(build_arg_list)
                .collect();
            Expr::Call {
                func: Box::new(object),
                args,
            }
        }
        Rule::index1_suffix => {
            let indices = pair
                .into_inner()
                .filter(|p| p.as_rule() == Rule::sub_list)
                .flat_map(build_sub_list)
                .collect();
            Expr::Index {
                object: Box::new(object),
                indices,
            }
        }
        Rule::index2_suffix => {
            let indices = pair
                .into_inner()
                .filter(|p| p.as_rule() == Rule::sub_list)
                .flat_map(build_sub_list)
                .collect();
            Expr::IndexDouble {
                object: Box::new(object),
                indices,
            }
        }
        Rule::dollar_suffix => {
            let inner = pair.into_inner().next().unwrap();
            let name = match inner.as_rule() {
                Rule::dots => "...".to_string(),
                _ => parse_ident_or_string(inner),
            };
            Expr::Dollar {
                object: Box::new(object),
                member: name,
            }
        }
        Rule::slot_suffix => {
            let inner = pair.into_inner().next().unwrap();
            let name = parse_ident_str(inner);
            Expr::Slot {
                object: Box::new(object),
                member: name,
            }
        }
        _ => unreachable!("unexpected postfix: {:?}", pair.as_rule()),
    }
}

// namespace_expr = { primary_expr ~ namespace_suffix* }
fn build_namespace_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(inner.next().unwrap());
    for suffix in inner {
        if suffix.as_rule() == Rule::namespace_suffix {
            let mut ns_inner = suffix.into_inner();
            let op_pair = ns_inner.next().unwrap(); // namespace_op
            let op_str = op_pair.as_str();
            let name_pair = ns_inner.next().unwrap();
            let name = parse_ident_or_string(name_pair);
            expr = if op_str == ":::" {
                Expr::NsGetInt {
                    namespace: Box::new(expr),
                    name,
                }
            } else {
                Expr::NsGet {
                    namespace: Box::new(expr),
                    name,
                }
            };
        }
    }
    expr
}

fn build_primary(pair: Pair<Rule>) -> Expr {
    let pair = match pair.as_rule() {
        Rule::primary_expr | Rule::keyword_constant => pair.into_inner().next().unwrap(),
        _ => pair,
    };

    match pair.as_rule() {
        Rule::null_lit => Expr::Null,
        Rule::na_lit => {
            let s = pair.as_str();
            let na_type = if s.starts_with("NA_complex") {
                NaType::Complex
            } else if s.starts_with("NA_character") {
                NaType::Character
            } else if s.starts_with("NA_real") {
                NaType::Real
            } else if s.starts_with("NA_integer") {
                NaType::Integer
            } else {
                NaType::Logical
            };
            Expr::Na(na_type)
        }
        Rule::inf_lit => Expr::Inf,
        Rule::nan_lit => Expr::NaN,
        Rule::bool_lit => {
            let val = pair.as_str().starts_with('T');
            Expr::Bool(val)
        }
        Rule::complex_number => parse_complex(pair),
        Rule::number => parse_number(pair),
        Rule::raw_string => parse_raw_string(pair),
        Rule::string => parse_string(pair),
        Rule::dots => Expr::Dots,
        Rule::dotdot => {
            let s = pair.as_str();
            let n: u32 = s[2..].parse().unwrap_or(1);
            Expr::DotDot(n)
        }
        Rule::ident => {
            let name = parse_ident_str(pair);
            Expr::Symbol(name)
        }
        Rule::if_expr => build_if(pair),
        Rule::for_expr => build_for(pair),
        Rule::while_expr => build_while(pair),
        Rule::repeat_expr => {
            let body = pair
                .into_inner()
                .find(|p| p.as_rule() == Rule::expr)
                .map(build_expr)
                .unwrap_or(Expr::Null);
            Expr::Repeat {
                body: Box::new(body),
            }
        }
        Rule::break_expr => Expr::Break,
        Rule::next_expr => Expr::Next,
        Rule::return_expr => {
            let val = pair
                .into_inner()
                .find(|p| p.as_rule() == Rule::expr)
                .map(|p| Box::new(build_expr(p)));
            Expr::Return(val)
        }
        Rule::function_def | Rule::lambda_def => build_function(pair),
        Rule::block => build_block(pair),
        Rule::paren_expr => {
            let inner = pair
                .into_inner()
                .find(|p| p.as_rule() == Rule::expr)
                .unwrap();
            build_expr(inner)
        }
        _ => build_expr(pair),
    }
}

fn build_if(pair: Pair<Rule>) -> Expr {
    let mut exprs: Vec<Expr> = pair
        .into_inner()
        .filter(|p| p.as_rule() == Rule::expr)
        .map(build_expr)
        .collect();
    let condition = exprs.remove(0);
    let then_body = exprs.remove(0);
    let else_body = if !exprs.is_empty() {
        Some(Box::new(exprs.remove(0)))
    } else {
        None
    };
    Expr::If {
        condition: Box::new(condition),
        then_body: Box::new(then_body),
        else_body,
    }
}

fn build_for(pair: Pair<Rule>) -> Expr {
    let inner = pair.into_inner();
    // Find ident and exprs
    let mut var = String::new();
    let mut exprs = Vec::new();
    for p in inner {
        match p.as_rule() {
            Rule::ident => var = parse_ident_str(p),
            Rule::expr => exprs.push(build_expr(p)),
            _ => {}
        }
    }
    let iter = exprs.remove(0);
    let body = exprs.remove(0);
    Expr::For {
        var,
        iter: Box::new(iter),
        body: Box::new(body),
    }
}

fn build_while(pair: Pair<Rule>) -> Expr {
    let exprs: Vec<Expr> = pair
        .into_inner()
        .filter(|p| p.as_rule() == Rule::expr)
        .map(build_expr)
        .collect();
    Expr::While {
        condition: Box::new(exprs[0].clone()),
        body: Box::new(exprs[1].clone()),
    }
}

fn build_function(pair: Pair<Rule>) -> Expr {
    let inner = pair.into_inner();
    let mut params = Vec::new();
    let mut body = None;

    for p in inner {
        match p.as_rule() {
            Rule::param_list => {
                params = build_param_list(p);
            }
            Rule::expr => {
                body = Some(build_expr(p));
            }
            _ => {}
        }
    }

    Expr::Function {
        params,
        body: Box::new(body.unwrap_or(Expr::Null)),
    }
}

fn build_param_list(pair: Pair<Rule>) -> Vec<Param> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::param)
        .map(|p| {
            let mut inner = p.into_inner();
            let first = inner.next().unwrap();
            if first.as_rule() == Rule::dots {
                Param {
                    name: "...".to_string(),
                    default: None,
                    is_dots: true,
                }
            } else {
                let name = parse_ident_str(first);
                // Check for = and default value
                let default = inner.find(|p| p.as_rule() == Rule::expr).map(build_expr);
                Param {
                    name,
                    default,
                    is_dots: false,
                }
            }
        })
        .collect()
}

fn build_block(pair: Pair<Rule>) -> Expr {
    let mut exprs = Vec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr_seq => {
                for child in p.into_inner() {
                    if child.as_rule() == Rule::expr {
                        exprs.push(build_expr(child));
                    }
                }
            }
            Rule::expr => exprs.push(build_expr(p)),
            _ => {}
        }
    }
    if exprs.is_empty() {
        Expr::Null
    } else {
        Expr::Block(exprs)
    }
}

// -------------------- argument lists --------------------

fn build_arg_list(pair: Pair<Rule>) -> Vec<Arg> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::arg_slot)
        .map(|slot| {
            match slot.into_inner().next() {
                None => Arg {
                    name: None,
                    value: None,
                }, // empty arg
                Some(arg_pair) => build_arg(arg_pair),
            }
        })
        .collect()
}

fn build_sub_list(pair: Pair<Rule>) -> Vec<Arg> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::sub_slot)
        .map(|slot| {
            match slot.into_inner().next() {
                None => Arg {
                    name: None,
                    value: None,
                }, // empty slot
                Some(sub_pair) => build_sub_arg(sub_pair),
            }
        })
        .collect()
}

fn build_arg(pair: Pair<Rule>) -> Arg {
    match pair.as_rule() {
        Rule::arg => {
            let inner_pair = pair.into_inner().next().unwrap();
            match inner_pair.as_rule() {
                Rule::named_arg => build_named_arg(inner_pair),
                _ => Arg {
                    name: None,
                    value: Some(build_expr(inner_pair)),
                },
            }
        }
        _ => Arg {
            name: None,
            value: Some(build_expr(pair)),
        },
    }
}

fn build_sub_arg(pair: Pair<Rule>) -> Arg {
    match pair.as_rule() {
        Rule::sub_arg => {
            let inner_pair = pair.into_inner().next().unwrap();
            match inner_pair.as_rule() {
                Rule::named_sub_arg => build_named_arg(inner_pair),
                _ => Arg {
                    name: None,
                    value: Some(build_expr(inner_pair)),
                },
            }
        }
        _ => Arg {
            name: None,
            value: Some(build_expr(pair)),
        },
    }
}

fn build_named_arg(pair: Pair<Rule>) -> Arg {
    let mut inner = pair.into_inner();
    let name_pair = inner.next().unwrap(); // arg_name
    let name = match name_pair.as_rule() {
        Rule::arg_name => {
            let inner_name = name_pair.into_inner().next().unwrap();
            match inner_name.as_rule() {
                Rule::dots => "...".to_string(),
                Rule::dotdot => inner_name.as_str().to_string(),
                Rule::string => parse_string_value(inner_name),
                _ => parse_ident_str(inner_name),
            }
        }
        _ => parse_ident_str(name_pair),
    };
    // Skip named_eq token
    let value = inner.find(|p| p.as_rule() == Rule::expr).map(build_expr);
    Arg {
        name: Some(name),
        value,
    }
}

// -------------------- number parsing --------------------

fn parse_complex(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // Remove trailing 'i'
    let num_str = &s[..s.len() - 1];
    let val = num_str.parse::<f64>().unwrap_or(0.0);
    Expr::Complex(val)
}

fn parse_number(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // Integer literal (ends with L)
    if let Some(num_str) = s.strip_suffix('L') {
        if num_str.starts_with("0x") || num_str.starts_with("0X") {
            return parse_hex_int(num_str);
        }
        if let Ok(val) = num_str.parse::<i64>() {
            return Expr::Integer(val);
        }
        if let Ok(val) = num_str.parse::<f64>() {
            // Intentional truncation: R `as.integer()` semantics for e.g. 1e5L
            return Expr::Integer(crate::interpreter::coerce::f64_to_i64(val).unwrap_or(0));
        }
    }
    // Hex (without L)
    if s.starts_with("0x") || s.starts_with("0X") {
        return parse_hex_float(s);
    }
    // Float / bare integer
    if let Ok(val) = s.parse::<f64>() {
        // In R, bare integers are still doubles unless suffixed with L
        return Expr::Double(val);
    }
    Expr::Double(0.0)
}

fn parse_hex_int(num_str: &str) -> Expr {
    let hex_part = &num_str[2..];
    // Check for hex float with '.' or 'p'
    if hex_part.contains('.') || hex_part.contains('p') || hex_part.contains('P') {
        let val = parse_hex_float_value(num_str);
        // Intentional truncation: hex float → integer literal (e.g. 0x1.0p4L)
        return Expr::Integer(crate::interpreter::coerce::f64_to_i64(val).unwrap_or(0));
    }
    let val = i64::from_str_radix(hex_part, 16).unwrap_or(0);
    Expr::Integer(val)
}

fn parse_hex_float(s: &str) -> Expr {
    let val = parse_hex_float_value(s);
    Expr::Double(val)
}

fn parse_hex_float_value(s: &str) -> f64 {
    let s = s.strip_suffix('L').unwrap_or(s);
    let hex_part = &s[2..]; // skip 0x/0X

    if let Some(p_pos) = hex_part.find(['p', 'P']) {
        let mantissa_str = &hex_part[..p_pos];
        let exp_str = &hex_part[p_pos + 1..];

        let mantissa = if let Some(dot_pos) = mantissa_str.find('.') {
            let int_part = &mantissa_str[..dot_pos];
            let frac_part = &mantissa_str[dot_pos + 1..];
            let int_val = if int_part.is_empty() {
                0u64
            } else {
                u64::from_str_radix(int_part, 16).unwrap_or(0)
            };
            let frac_val = if frac_part.is_empty() {
                0.0
            } else {
                let frac_int = u64::from_str_radix(frac_part, 16).unwrap_or(0);
                // u64 → f64 may lose precision for values > 2^53, acceptable for hex literals
                let frac_digits = i32::try_from(frac_part.len()).unwrap_or(0);
                crate::interpreter::coerce::u64_to_f64(frac_int) / 16f64.powi(frac_digits)
            };
            crate::interpreter::coerce::u64_to_f64(int_val) + frac_val
        } else {
            crate::interpreter::coerce::u64_to_f64(
                u64::from_str_radix(mantissa_str, 16).unwrap_or(0),
            )
        };

        let exp: i32 = exp_str.parse().unwrap_or(0);
        mantissa * 2f64.powi(exp)
    } else if let Some(dot_pos) = hex_part.find('.') {
        // Hex with dot but no exponent
        let int_part = &hex_part[..dot_pos];
        let frac_part = &hex_part[dot_pos + 1..];
        let int_val = if int_part.is_empty() {
            0u64
        } else {
            u64::from_str_radix(int_part, 16).unwrap_or(0)
        };
        let frac_val = if frac_part.is_empty() {
            0.0
        } else {
            let frac_int = u64::from_str_radix(frac_part, 16).unwrap_or(0);
            let frac_digits = i32::try_from(frac_part.len()).unwrap_or(0);
            crate::interpreter::coerce::u64_to_f64(frac_int) / 16f64.powi(frac_digits)
        };
        crate::interpreter::coerce::u64_to_f64(int_val) + frac_val
    } else {
        crate::interpreter::coerce::i64_to_f64(i64::from_str_radix(hex_part, 16).unwrap_or(0))
    }
}

fn parse_raw_string(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // r"(...)" or R'(...)' etc - find the body between delimiters
    // Skip r/R and the quote char, then the opening delimiter
    let quote_pos = s.find('"').or_else(|| s.find('\'')).unwrap();
    let inner = &s[quote_pos + 1..s.len() - 1]; // between outer quotes
                                                // inner is like "(...)" — strip the delimiter pair
    let content = if inner.starts_with('(') || inner.starts_with('[') || inner.starts_with('{') {
        &inner[1..inner.len() - 1]
    } else {
        inner
    };
    Expr::String(content.to_string())
}

fn parse_string_value(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    let inner = &s[1..s.len() - 1];
    unescape_string(inner)
}

fn parse_string(pair: Pair<Rule>) -> Expr {
    Expr::String(parse_string_value(pair))
}

fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('0') => result.push('\0'),
                Some('a') => result.push('\x07'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('v') => result.push('\x0B'),
                Some('x') => {
                    let hex: String = chars.clone().take(2).collect();
                    if let Ok(val) = u8::from_str_radix(&hex, 16) {
                        result.push(val as char);
                        chars.nth(1);
                    }
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn parse_ident_str(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    if s.starts_with('`') && s.ends_with('`') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_ident_or_string(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    if s.starts_with('`') && s.ends_with('`') {
        s[1..s.len() - 1].to_string()
    } else if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    {
        unescape_string(&s[1..s.len() - 1])
    } else {
        s.to_string()
    }
}
