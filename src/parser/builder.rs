//! AST builder functions — converts pest `Pair`s into `Expr` nodes.

use pest::iterators::Pair;

use super::ast::*;
use super::literals::{parse_complex, parse_number, parse_raw_string, parse_string};
use super::Rule;

// region: Identifier helpers

fn unescape_backtick_ident(s: &str) -> String {
    s.replace("\\`", "`")
}

pub(super) fn parse_ident_str(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    if s.starts_with('`') && s.ends_with('`') {
        unescape_backtick_ident(&s[1..s.len() - 1])
    } else {
        s.to_string()
    }
}

pub(super) fn parse_ident_or_string(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    if s.starts_with('`') && s.ends_with('`') {
        unescape_backtick_ident(&s[1..s.len() - 1])
    } else if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    {
        super::literals::unescape_string(&s[1..s.len() - 1])
    } else {
        s.to_string()
    }
}

// endregion

// region: Program and expression dispatch

pub(super) fn build_program(pair: Pair<Rule>) -> Expr {
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

pub(super) fn build_expr(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::expr => build_expr(pair.into_inner().next().unwrap()),
        Rule::help_expr => build_help(pair),
        Rule::assign_eq_expr => build_assign_eq(pair),
        Rule::walrus_expr => build_walrus(pair),
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

// endregion

// region: Help expression

fn build_help(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    if first.as_rule() == Rule::help_expr {
        // Unary: "?foo" -> help("foo")
        let topic = extract_help_topic(&first);
        // Wrap in invisible() so ?foo doesn't print NULL
        Expr::Call {
            func: Box::new(Expr::Symbol("invisible".to_string())),
            args: vec![Arg {
                name: None,
                value: Some(Expr::Call {
                    func: Box::new(Expr::Symbol("help".to_string())),
                    args: vec![Arg {
                        name: None,
                        value: Some(Expr::String(topic)),
                    }],
                }),
            }],
        }
    } else {
        // Binary: expr ~ "?" ~ expr -- just evaluate the LHS
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
    text.to_string()
}

// endregion

// region: Assignment expressions

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

fn build_walrus(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let lhs = build_expr(inner.next().unwrap());
    match inner.next() {
        None => lhs,
        Some(op_pair) => {
            assert!(op_pair.as_rule() == Rule::walrus_assign_op);
            let rhs = build_expr(inner.next().unwrap());
            Expr::BinaryOp {
                op: BinaryOp::Special(SpecialOp::Walrus),
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }
        }
    }
}

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

// endregion

// region: Formula

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
        let lhs = build_expr(first);
        match inner.next() {
            None => lhs,
            Some(op_pair) => {
                let rhs = build_expr(inner.next().unwrap());
                let remaining: Vec<_> = inner.collect();

                if op_pair.as_rule() == Rule::tilde_op
                    && op_pair.as_str() == "~"
                    && remaining.is_empty()
                {
                    return Expr::Formula {
                        lhs: Some(Box::new(lhs)),
                        rhs: Some(Box::new(rhs)),
                    };
                }

                let mut expr = Expr::BinaryOp {
                    op: map_tilde_op(&op_pair),
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                };

                let mut remaining = remaining.into_iter();
                while let Some(next_op) = remaining.next() {
                    let next_rhs = build_expr(remaining.next().unwrap());
                    expr = Expr::BinaryOp {
                        op: map_tilde_op(&next_op),
                        lhs: Box::new(expr),
                        rhs: Box::new(next_rhs),
                    };
                }

                expr
            }
        }
    }
}

fn map_tilde_op(pair: &Pair<Rule>) -> BinaryOp {
    match pair.as_str() {
        "~" => BinaryOp::Tilde,
        "~~" => BinaryOp::DoubleTilde,
        _ => unreachable!(),
    }
}

// endregion

// region: Binary and unary operators

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

// endregion

// region: Postfix and namespace expressions

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

// endregion

// region: Primary expressions

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
        Rule::formula_literal => {
            let rhs = pair
                .into_inner()
                .next()
                .map(build_expr)
                .unwrap_or(Expr::Null);
            Expr::Formula {
                lhs: None,
                rhs: Some(Box::new(rhs)),
            }
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

// endregion

// region: Control flow

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

// endregion

// region: Argument lists

fn build_arg_list(pair: Pair<Rule>) -> Vec<Arg> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::arg_slot)
        .map(|slot| {
            match slot.into_inner().next() {
                None => Arg {
                    name: None,
                    value: None,
                }, // empty arg
                Some(arg_pair) => build_arg_or_sub(arg_pair),
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
                Some(sub_pair) => build_arg_or_sub(sub_pair),
            }
        })
        .collect()
}

/// Shared logic for both call args and index args — structurally identical.
fn build_arg_or_sub(pair: Pair<Rule>) -> Arg {
    let inner_pair = pair.into_inner().next().unwrap();
    match inner_pair.as_rule() {
        Rule::named_arg | Rule::named_sub_arg => build_named_arg(inner_pair),
        _ => Arg {
            name: None,
            value: Some(build_expr(inner_pair)),
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
                Rule::string => super::literals::parse_string_value(inner_name),
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

// endregion
