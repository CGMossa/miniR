pub mod builtins;
pub mod environment;
pub mod value;

use crate::parser::ast::*;
use environment::Environment;
use value::*;

/// Extract generic name from a UseMethod("name") call in a function body.
/// Handles: UseMethod("name"), { UseMethod("name") }, { ...; UseMethod("name") }
fn extract_use_method(body: &Expr) -> Option<String> {
    match body {
        Expr::Call { func, args } => {
            if let Expr::Symbol(name) = func.as_ref() {
                if name == "UseMethod" {
                    if let Some(arg) = args.first() {
                        if let Some(Expr::String(s)) = arg.value.as_ref() {
                            return Some(s.clone());
                        }
                    }
                }
            }
            None
        }
        Expr::Block(stmts) => {
            // Check last statement in block
            stmts.last().and_then(extract_use_method)
        }
        _ => None,
    }
}

pub struct Interpreter {
    pub global_env: Environment,
}

impl Interpreter {
    pub fn new() -> Self {
        let base_env = Environment::new_global();
        builtins::register_builtins(&base_env);
        let global_env = Environment::new_child(&base_env);
        Interpreter { global_env }
    }

    pub fn eval(&mut self, expr: &Expr) -> Result<RValue, RError> {
        self.eval_in(expr, &self.global_env.clone())
    }

    pub fn eval_in(&mut self, expr: &Expr, env: &Environment) -> Result<RValue, RError> {
        match expr {
            Expr::Null => Ok(RValue::Null),
            Expr::Na(na_type) => Ok(match na_type {
                NaType::Logical => RValue::Vector(Vector::Logical(vec![None])),
                NaType::Integer => RValue::Vector(Vector::Integer(vec![None])),
                NaType::Real => RValue::Vector(Vector::Double(vec![None])),
                NaType::Character => RValue::Vector(Vector::Character(vec![None])),
                NaType::Complex => RValue::Vector(Vector::Double(vec![None])),
            }),
            Expr::Inf => Ok(RValue::Vector(Vector::Double(vec![Some(f64::INFINITY)]))),
            Expr::NaN => Ok(RValue::Vector(Vector::Double(vec![Some(f64::NAN)]))),
            Expr::Bool(b) => Ok(RValue::Vector(Vector::Logical(vec![Some(*b)]))),
            Expr::Integer(i) => Ok(RValue::Vector(Vector::Integer(vec![Some(*i)]))),
            Expr::Double(f) => Ok(RValue::Vector(Vector::Double(vec![Some(*f)]))),
            Expr::String(s) => Ok(RValue::Vector(Vector::Character(vec![Some(s.clone())]))),
            Expr::Complex(f) => Ok(RValue::Vector(Vector::Double(vec![Some(*f)]))), // stub: treat as double
            Expr::Symbol(name) => env.get(name).ok_or_else(|| RError::Name(name.clone())),
            Expr::Dots => Ok(RValue::Null),
            Expr::DotDot(_) => Ok(RValue::Null), // stub for ..1, ..2 etc.

            Expr::UnaryOp { op, operand } => {
                let val = self.eval_in(operand, env)?;
                self.eval_unary(*op, &val)
            }
            Expr::BinaryOp { op, lhs, rhs } => {
                // Special handling for pipe
                if matches!(op, BinaryOp::Pipe) {
                    return self.eval_pipe(lhs, rhs, env);
                }
                let left = self.eval_in(lhs, env)?;
                let right = self.eval_in(rhs, env)?;
                self.eval_binary(*op, &left, &right)
            }
            Expr::Assign { op, target, value } => {
                let val = self.eval_in(value, env)?;
                self.eval_assign(op, target, val, env)
            }

            Expr::Call { func, args } => self.eval_call(func, args, env),
            Expr::Index { object, indices } => self.eval_index(object, indices, env),
            Expr::IndexDouble { object, indices } => self.eval_index_double(object, indices, env),
            Expr::Dollar { object, member } => self.eval_dollar(object, member, env),
            Expr::Slot { object, member } => self.eval_dollar(object, member, env), // treat like $
            Expr::NsGet { namespace, name } => self.eval_ns_get(namespace, name, env),
            Expr::NsGetInt { namespace, name } => self.eval_ns_get(namespace, name, env),

            Expr::Formula { lhs: _, rhs: _ } => Ok(RValue::Null), // Stub for formula

            Expr::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond = self.eval_in(condition, env)?;
                let test = match &cond {
                    RValue::Vector(v) => v.as_logical_scalar(),
                    _ => None,
                };
                match test {
                    Some(true) => self.eval_in(then_body, env),
                    Some(false) | None => {
                        if let Some(else_expr) = else_body {
                            self.eval_in(else_expr, env)
                        } else {
                            Ok(RValue::Null)
                        }
                    }
                }
            }

            Expr::For { var, iter, body } => {
                let iter_val = self.eval_in(iter, env)?;
                self.eval_for(var, &iter_val, body, env)
            }

            Expr::While { condition, body } => {
                loop {
                    let cond = self.eval_in(condition, env)?;
                    let test = match &cond {
                        RValue::Vector(v) => v.as_logical_scalar().unwrap_or(false),
                        _ => false,
                    };
                    if !test {
                        break;
                    }
                    match self.eval_in(body, env) {
                        Err(RError::Break) => break,
                        Err(RError::Next) => continue,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(RValue::Null)
            }

            Expr::Repeat { body } => {
                loop {
                    match self.eval_in(body, env) {
                        Err(RError::Break) => break,
                        Err(RError::Next) => continue,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(RValue::Null)
            }

            Expr::Break => Err(RError::Break),
            Expr::Next => Err(RError::Next),
            Expr::Return(val) => {
                let ret_val = match val {
                    Some(expr) => self.eval_in(expr, env)?,
                    None => RValue::Null,
                };
                Err(RError::Return(ret_val))
            }

            Expr::Block(exprs) => {
                let mut result = RValue::Null;
                for expr in exprs {
                    result = self.eval_in(expr, env)?;
                }
                Ok(result)
            }

            Expr::Function { params, body } => Ok(RValue::Function(RFunction::Closure {
                params: params.clone(),
                body: (**body).clone(),
                env: env.clone(),
            })),

            Expr::Program(exprs) => {
                let mut result = RValue::Null;
                for expr in exprs {
                    result = self.eval_in(expr, env)?;
                }
                Ok(result)
            }
        }
    }

    fn eval_unary(&self, op: UnaryOp, val: &RValue) -> Result<RValue, RError> {
        match op {
            UnaryOp::Neg => match val {
                RValue::Vector(v) => {
                    let result = match v {
                        Vector::Double(vals) => {
                            Vector::Double(vals.iter().map(|x| x.map(|f| -f)).collect())
                        }
                        Vector::Integer(vals) => {
                            Vector::Integer(vals.iter().map(|x| x.map(|i| -i)).collect())
                        }
                        Vector::Logical(vals) => Vector::Integer(
                            vals.iter()
                                .map(|x| x.map(|b| if b { -1 } else { 0 }))
                                .collect(),
                        ),
                        _ => {
                            return Err(RError::Type(
                                "invalid argument to unary operator".to_string(),
                            ))
                        }
                    };
                    Ok(RValue::Vector(result))
                }
                _ => Err(RError::Type(
                    "invalid argument to unary operator".to_string(),
                )),
            },
            UnaryOp::Pos => Ok(val.clone()),
            UnaryOp::Not => match val {
                RValue::Vector(v) => {
                    let logicals = v.to_logicals();
                    let result: Vec<Option<bool>> =
                        logicals.iter().map(|x| x.map(|b| !b)).collect();
                    Ok(RValue::Vector(Vector::Logical(result)))
                }
                _ => Err(RError::Type("invalid argument type".to_string())),
            },
            UnaryOp::Formula => Ok(RValue::Null), // stub for unary ~
        }
    }

    fn eval_binary(&self, op: BinaryOp, left: &RValue, right: &RValue) -> Result<RValue, RError> {
        match op {
            BinaryOp::Range => return self.eval_range(left, right),
            BinaryOp::Special(SpecialOp::In) => return self.eval_in_op(left, right),
            _ => {}
        };

        // Get vectors for element-wise operations
        let lv = match left {
            RValue::Vector(v) => v,
            RValue::Null => return Ok(RValue::Null),
            _ => {
                return Err(RError::Type(
                    "non-numeric argument to binary operator".to_string(),
                ))
            }
        };
        let rv = match right {
            RValue::Vector(v) => v,
            RValue::Null => return Ok(RValue::Null),
            _ => {
                return Err(RError::Type(
                    "non-numeric argument to binary operator".to_string(),
                ))
            }
        };

        match op {
            BinaryOp::Range => self.eval_range(left, right),
            BinaryOp::Special(SpecialOp::In) => self.eval_in_op(left, right),
            BinaryOp::Special(_) => Ok(RValue::Null),

            // Arithmetic (vectorized with recycling)
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Pow
            | BinaryOp::Mod
            | BinaryOp::IntDiv => self.eval_arith(op, lv, rv),

            // Comparison (vectorized)
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Le
            | BinaryOp::Ge => self.eval_compare(op, lv, rv),

            // Logical (vectorized)
            BinaryOp::And | BinaryOp::Or => self.eval_logical_vec(op, lv, rv),

            // Scalar logical
            BinaryOp::AndScalar => {
                let a = lv.as_logical_scalar();
                let b = rv.as_logical_scalar();
                match (a, b) {
                    (Some(false), _) | (_, Some(false)) => {
                        Ok(RValue::Vector(Vector::Logical(vec![Some(false)])))
                    }
                    (Some(true), Some(true)) => {
                        Ok(RValue::Vector(Vector::Logical(vec![Some(true)])))
                    }
                    _ => Ok(RValue::Vector(Vector::Logical(vec![None]))),
                }
            }
            BinaryOp::OrScalar => {
                let a = lv.as_logical_scalar();
                let b = rv.as_logical_scalar();
                match (a, b) {
                    (Some(true), _) | (_, Some(true)) => {
                        Ok(RValue::Vector(Vector::Logical(vec![Some(true)])))
                    }
                    (Some(false), Some(false)) => {
                        Ok(RValue::Vector(Vector::Logical(vec![Some(false)])))
                    }
                    _ => Ok(RValue::Vector(Vector::Logical(vec![None]))),
                }
            }

            BinaryOp::Pipe => unreachable!("pipe handled separately"),
            BinaryOp::Tilde => Ok(RValue::Null), // stub for binary ~
        }
    }

    fn eval_arith(&self, op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RError> {
        // Check if both are integer and op preserves integer type
        let use_integer = matches!(
            (&lv, &rv, &op),
            (
                Vector::Integer(_),
                Vector::Integer(_),
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::IntDiv | BinaryOp::Mod
            )
        );

        if use_integer {
            let li = lv.to_integers();
            let ri = rv.to_integers();
            let len = li.len().max(ri.len());
            if len == 0 {
                return Ok(RValue::Vector(Vector::Integer(vec![])));
            }
            let result: Vec<Option<i64>> = (0..len)
                .map(|i| {
                    let a = li[i % li.len()];
                    let b = ri[i % ri.len()];
                    match (a, b) {
                        (Some(a), Some(b)) => match op {
                            BinaryOp::Add => Some(a.wrapping_add(b)),
                            BinaryOp::Sub => Some(a.wrapping_sub(b)),
                            BinaryOp::Mul => Some(a.wrapping_mul(b)),
                            BinaryOp::IntDiv => {
                                if b != 0 {
                                    Some(a / b)
                                } else {
                                    None
                                }
                            }
                            BinaryOp::Mod => {
                                if b != 0 {
                                    Some(a % b)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                })
                .collect();
            return Ok(RValue::Vector(Vector::Integer(result)));
        }

        let ld = lv.to_doubles();
        let rd = rv.to_doubles();
        let len = ld.len().max(rd.len());
        if len == 0 {
            return Ok(RValue::Vector(Vector::Double(vec![])));
        }

        let result: Vec<Option<f64>> = (0..len)
            .map(|i| {
                let a = ld[i % ld.len()];
                let b = rd[i % rd.len()];
                match (a, b) {
                    (Some(a), Some(b)) => Some(match op {
                        BinaryOp::Add => a + b,
                        BinaryOp::Sub => a - b,
                        BinaryOp::Mul => a * b,
                        BinaryOp::Div => a / b,
                        BinaryOp::Pow => a.powf(b),
                        BinaryOp::Mod => a % b,
                        BinaryOp::IntDiv => (a / b).floor(),
                        _ => unreachable!(),
                    }),
                    _ => None,
                }
            })
            .collect();
        Ok(RValue::Vector(Vector::Double(result)))
    }

    fn eval_compare(&self, op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RError> {
        // If either is character, compare as strings
        if matches!(lv, Vector::Character(_)) || matches!(rv, Vector::Character(_)) {
            let lc = lv.to_characters();
            let rc = rv.to_characters();
            let len = lc.len().max(rc.len());
            let result: Vec<Option<bool>> = (0..len)
                .map(|i| {
                    let a = &lc[i % lc.len()];
                    let b = &rc[i % rc.len()];
                    match (a, b) {
                        (Some(a), Some(b)) => Some(match op {
                            BinaryOp::Eq => a == b,
                            BinaryOp::Ne => a != b,
                            BinaryOp::Lt => a < b,
                            BinaryOp::Gt => a > b,
                            BinaryOp::Le => a <= b,
                            BinaryOp::Ge => a >= b,
                            _ => unreachable!(),
                        }),
                        _ => None,
                    }
                })
                .collect();
            return Ok(RValue::Vector(Vector::Logical(result)));
        }

        let ld = lv.to_doubles();
        let rd = rv.to_doubles();
        let len = ld.len().max(rd.len());
        if len == 0 {
            return Ok(RValue::Vector(Vector::Logical(vec![])));
        }

        let result: Vec<Option<bool>> = (0..len)
            .map(|i| {
                let a = ld[i % ld.len()];
                let b = rd[i % rd.len()];
                match (a, b) {
                    (Some(a), Some(b)) => Some(match op {
                        BinaryOp::Eq => a == b,
                        BinaryOp::Ne => a != b,
                        BinaryOp::Lt => a < b,
                        BinaryOp::Gt => a > b,
                        BinaryOp::Le => a <= b,
                        BinaryOp::Ge => a >= b,
                        _ => unreachable!(),
                    }),
                    _ => None,
                }
            })
            .collect();
        Ok(RValue::Vector(Vector::Logical(result)))
    }

    fn eval_logical_vec(&self, op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RError> {
        let ll = lv.to_logicals();
        let rl = rv.to_logicals();
        let len = ll.len().max(rl.len());

        let result: Vec<Option<bool>> = (0..len)
            .map(|i| {
                let a = ll[i % ll.len()];
                let b = rl[i % rl.len()];
                match op {
                    BinaryOp::And => match (a, b) {
                        (Some(false), _) | (_, Some(false)) => Some(false),
                        (Some(true), Some(true)) => Some(true),
                        _ => None,
                    },
                    BinaryOp::Or => match (a, b) {
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        (Some(false), Some(false)) => Some(false),
                        _ => None,
                    },
                    _ => unreachable!(),
                }
            })
            .collect();
        Ok(RValue::Vector(Vector::Logical(result)))
    }

    fn eval_range(&self, left: &RValue, right: &RValue) -> Result<RValue, RError> {
        let from = match left {
            RValue::Vector(v) => v.as_double_scalar().unwrap_or(0.0) as i64,
            _ => 0,
        };
        let to = match right {
            RValue::Vector(v) => v.as_double_scalar().unwrap_or(0.0) as i64,
            _ => 0,
        };

        let result: Vec<Option<i64>> = if from <= to {
            (from..=to).map(Some).collect()
        } else {
            (to..=from).rev().map(Some).collect()
        };
        Ok(RValue::Vector(Vector::Integer(result)))
    }

    fn eval_in_op(&self, left: &RValue, right: &RValue) -> Result<RValue, RError> {
        let table = match right {
            RValue::Vector(v) => v.to_characters(),
            _ => vec![],
        };
        match left {
            RValue::Vector(v) => {
                let chars = v.to_characters();
                let result: Vec<Option<bool>> =
                    chars.iter().map(|x| Some(table.contains(x))).collect();
                Ok(RValue::Vector(Vector::Logical(result)))
            }
            _ => Ok(RValue::Vector(Vector::Logical(vec![Some(false)]))),
        }
    }

    fn eval_pipe(&mut self, lhs: &Expr, rhs: &Expr, env: &Environment) -> Result<RValue, RError> {
        let left_val = self.eval_in(lhs, env)?;
        // rhs should be a function call; inject left_val as first argument
        match rhs {
            Expr::Call { func, args } => {
                let f = self.eval_in(func, env)?;
                let mut eval_args = vec![left_val];
                let mut named_args = Vec::new();
                for arg in args {
                    if let Some(ref name) = arg.name {
                        if let Some(ref val_expr) = arg.value {
                            named_args.push((name.clone(), self.eval_in(val_expr, env)?));
                        }
                    } else if let Some(ref val_expr) = arg.value {
                        eval_args.push(self.eval_in(val_expr, env)?);
                    }
                }
                self.call_function(&f, &eval_args, &named_args, env)
            }
            Expr::Symbol(name) => {
                // x |> f  is equivalent to f(x)
                let f = env.get(name).ok_or_else(|| RError::Name(name.clone()))?;
                self.call_function(&f, &[left_val], &[], env)
            }
            _ => Err(RError::Other("invalid use of pipe".to_string())),
        }
    }

    fn eval_assign(
        &mut self,
        op: &AssignOp,
        target: &Expr,
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RError> {
        match target {
            Expr::Symbol(name) => {
                match op {
                    AssignOp::SuperAssign | AssignOp::RightSuperAssign => {
                        env.set_super(name.clone(), val.clone());
                    }
                    _ => {
                        env.set(name.clone(), val.clone());
                    }
                }
                Ok(val)
            }
            // Assignment to index: x[i] <- val
            Expr::Index { object, indices } => self.eval_index_assign(object, indices, val, env),
            Expr::IndexDouble { object, indices } => {
                self.eval_index_double_assign(object, indices, val, env)
            }
            Expr::Dollar { object, member } => self.eval_dollar_assign(object, member, val, env),
            // Handle function calls on left side like names(x) <- val
            Expr::Call { func, args } => {
                if let Expr::Symbol(fname) = func.as_ref() {
                    let replacement_fn = format!("{}<-", fname);
                    if let Some(arg) = args.first() {
                        if let Some(ref val_expr) = arg.value {
                            let obj = self.eval_in(val_expr, env)?;
                            // Try calling the replacement function
                            if let Some(f) = env.get(&replacement_fn) {
                                let result =
                                    self.call_function(&f, &[obj, val.clone()], &[], env)?;
                                // Assign result back to the variable
                                if let Expr::Symbol(var_name) = val_expr {
                                    env.set(var_name.clone(), result);
                                }
                                return Ok(val);
                            }
                        }
                    }
                }
                Err(RError::Other("invalid assignment target".to_string()))
            }
            _ => Err(RError::Other("invalid assignment target".to_string())),
        }
    }

    fn eval_call(
        &mut self,
        func: &Expr,
        args: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RError> {
        let f = self.eval_in(func, env)?;

        // R behavior: if the symbol resolved to a non-function but we're in
        // call position, search up the env chain for a function with that name
        // (like R's findFun). This lets `c <- 1; c(1,2,3)` still work.
        let f = if !matches!(f, RValue::Function(_)) {
            if let Expr::Symbol(name) = func {
                env.get_function(name)
                    .ok_or_else(|| RError::Other("attempt to apply non-function".to_string()))?
            } else {
                f
            }
        } else {
            f
        };

        let mut positional = Vec::new();
        let mut named = Vec::new();

        for arg in args {
            if let Some(ref name) = arg.name {
                if let Some(ref val_expr) = arg.value {
                    named.push((name.clone(), self.eval_in(val_expr, env)?));
                } else {
                    // name= with no value (missing)
                    named.push((name.clone(), RValue::Null));
                }
            } else if let Some(ref val_expr) = arg.value {
                positional.push(self.eval_in(val_expr, env)?);
            }
        }

        self.call_function(&f, &positional, &named, env)
    }

    fn call_function(
        &mut self,
        func: &RValue,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
    ) -> Result<RValue, RError> {
        match func {
            RValue::Function(RFunction::Builtin { func, name, .. }) => {
                // Handle apply functions specially
                match name.as_str() {
                    "sapply" | "lapply" => {
                        return self.eval_apply(positional, named, name == "sapply");
                    }
                    "vapply" => {
                        return self.eval_apply(positional, named, true);
                    }
                    "do.call" => {
                        if positional.len() >= 2 {
                            let f = &positional[0];
                            match &positional[1] {
                                RValue::List(l) => {
                                    let args: Vec<RValue> =
                                        l.values.iter().map(|(_, v)| v.clone()).collect();
                                    return self.call_function(f, &args, named, env);
                                }
                                _ => return self.call_function(f, &positional[1..], named, env),
                            }
                        }
                        return func(positional, named);
                    }
                    "Vectorize" => {
                        // Return the function as-is (simplified)
                        return Ok(positional.first().cloned().unwrap_or(RValue::Null));
                    }
                    "tryCatch" => {
                        return self.eval_try_catch(positional, named, env);
                    }
                    "try" => {
                        return Ok(positional.first().cloned().unwrap_or(RValue::Null));
                    }
                    _ => {}
                }
                func(positional, named)
            }
            RValue::Function(RFunction::Closure {
                params,
                body,
                env: closure_env,
            }) => {
                // Check for S3 generic (body contains UseMethod("generic"))
                if let Some(generic_name) = extract_use_method(body) {
                    return self.dispatch_s3(&generic_name, positional, named, env);
                }

                let call_env = Environment::new_child(closure_env);

                // Bind parameters
                let mut pos_idx = 0;
                let mut dots_vals: Vec<RValue> = Vec::new();

                for param in params {
                    if param.is_dots {
                        // Collect remaining positional args into ...
                        while pos_idx < positional.len() {
                            dots_vals.push(positional[pos_idx].clone());
                            pos_idx += 1;
                        }
                        continue;
                    }

                    // Try named argument first
                    if let Some((_, val)) = named.iter().find(|(n, _)| *n == param.name) {
                        call_env.set(param.name.clone(), val.clone());
                    } else if pos_idx < positional.len() {
                        call_env.set(param.name.clone(), positional[pos_idx].clone());
                        pos_idx += 1;
                    } else if let Some(ref default) = param.default {
                        let val = self.eval_in(default, &call_env)?;
                        call_env.set(param.name.clone(), val);
                    }
                    // else: missing argument, will error when accessed
                }

                match self.eval_in(body, &call_env) {
                    Ok(val) => Ok(val),
                    Err(RError::Return(val)) => Ok(val),
                    Err(e) => Err(e),
                }
            }
            _ => Err(RError::Type("attempt to apply non-function".to_string())),
        }
    }

    fn eval_apply(
        &mut self,
        positional: &[RValue],
        _named: &[(String, RValue)],
        simplify: bool,
    ) -> Result<RValue, RError> {
        if positional.len() < 2 {
            return Err(RError::Argument(
                "need at least 2 arguments for apply".to_string(),
            ));
        }
        let x = &positional[0];
        let f = &positional[1];

        let items: Vec<RValue> = match x {
            RValue::Vector(v) => match v {
                Vector::Double(vals) => vals
                    .iter()
                    .map(|x| RValue::Vector(Vector::Double(vec![*x])))
                    .collect(),
                Vector::Integer(vals) => vals
                    .iter()
                    .map(|x| RValue::Vector(Vector::Integer(vec![*x])))
                    .collect(),
                Vector::Character(vals) => vals
                    .iter()
                    .map(|x| RValue::Vector(Vector::Character(vec![x.clone()])))
                    .collect(),
                Vector::Logical(vals) => vals
                    .iter()
                    .map(|x| RValue::Vector(Vector::Logical(vec![*x])))
                    .collect(),
            },
            RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
            _ => vec![x.clone()],
        };

        let env = &self.global_env.clone();
        let mut results: Vec<RValue> = Vec::new();
        for item in &items {
            let result = self.call_function(f, std::slice::from_ref(item), &[], env)?;
            results.push(result);
        }

        if simplify {
            // Try to simplify to a vector
            let all_scalar = results.iter().all(|r| r.length() == 1);
            if all_scalar && !results.is_empty() {
                // Check if all are the same type
                let first_type = results[0].type_name();
                let all_same = results.iter().all(|r| r.type_name() == first_type);
                if all_same {
                    match first_type {
                        "double" => {
                            let vals: Vec<Option<f64>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::Vector(Vector::Double(vals)));
                        }
                        "integer" => {
                            let vals: Vec<Option<i64>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::Vector(Vector::Integer(vals)));
                        }
                        "character" => {
                            let vals: Vec<Option<String>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector().map(|v| {
                                        v.to_characters().into_iter().next().unwrap_or(None)
                                    })
                                })
                                .collect();
                            return Ok(RValue::Vector(Vector::Character(vals)));
                        }
                        "logical" => {
                            let vals: Vec<Option<bool>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::Vector(Vector::Logical(vals)));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Return as list
        let values: Vec<(Option<String>, RValue)> =
            results.into_iter().map(|v| (None, v)).collect();
        Ok(RValue::List(RList::new(values)))
    }

    fn eval_try_catch(
        &mut self,
        positional: &[RValue],
        _named: &[(String, RValue)],
        _env: &Environment,
    ) -> Result<RValue, RError> {
        // Just return the first argument (simplified tryCatch)
        Ok(positional.first().cloned().unwrap_or(RValue::Null))
    }

    fn eval_index(
        &mut self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RError> {
        let obj = self.eval_in(object, env)?;

        if indices.is_empty() {
            return Ok(obj);
        }

        // Evaluate indices
        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
        } else {
            return Ok(obj);
        };

        match &obj {
            RValue::Vector(v) => {
                match &idx_val {
                    RValue::Vector(idx_vec) => {
                        // Logical indexing
                        if let Vector::Logical(mask) = idx_vec {
                            return self.index_by_logical(v, mask);
                        }
                        // Negative indexing (exclusion)
                        let indices = idx_vec.to_integers();
                        if indices.iter().all(|x| x.map(|i| i < 0).unwrap_or(false)) {
                            return self.index_by_negative(v, &indices);
                        }
                        // Positive integer indexing
                        self.index_by_integer(v, &indices)
                    }
                    RValue::Null => Ok(obj.clone()),
                    _ => Err(RError::Index("invalid index type".to_string())),
                }
            }
            RValue::List(list) => {
                match &idx_val {
                    RValue::Vector(idx_vec) => {
                        // String indexing
                        if let Vector::Character(names) = idx_vec {
                            let mut result = Vec::new();
                            for name in names.iter().flatten() {
                                let found = list
                                    .values
                                    .iter()
                                    .find(|(n, _)| n.as_ref() == Some(name))
                                    .map(|(n, v)| (n.clone(), v.clone()));
                                if let Some(item) = found {
                                    result.push(item);
                                }
                            }
                            return Ok(RValue::List(RList::new(result)));
                        }
                        // Integer indexing
                        let indices = idx_vec.to_integers();
                        let mut result = Vec::new();
                        for i in indices.iter().flatten() {
                            let i = *i as usize;
                            if i > 0 && i <= list.values.len() {
                                result.push(list.values[i - 1].clone());
                            }
                        }
                        Ok(RValue::List(RList::new(result)))
                    }
                    _ => Err(RError::Index("invalid index type".to_string())),
                }
            }
            _ => Err(RError::Index("object is not subsettable".to_string())),
        }
    }

    fn index_by_integer(&self, v: &Vector, indices: &[Option<i64>]) -> Result<RValue, RError> {
        macro_rules! index_vec {
            ($vals:expr, $variant:ident) => {{
                let result: Vec<_> = indices
                    .iter()
                    .map(|idx| {
                        idx.and_then(|i| {
                            let i = i as usize;
                            if i > 0 && i <= $vals.len() {
                                $vals[i - 1].clone().into()
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                Ok(RValue::Vector(Vector::$variant(result)))
            }};
        }
        match v {
            Vector::Double(vals) => index_vec!(vals, Double),
            Vector::Integer(vals) => index_vec!(vals, Integer),
            Vector::Logical(vals) => index_vec!(vals, Logical),
            Vector::Character(vals) => index_vec!(vals, Character),
        }
    }

    fn index_by_negative(&self, v: &Vector, indices: &[Option<i64>]) -> Result<RValue, RError> {
        let exclude: Vec<usize> = indices
            .iter()
            .filter_map(|x| x.map(|i| (-i) as usize))
            .collect();

        macro_rules! filter_vec {
            ($vals:expr, $variant:ident) => {{
                let result: Vec<_> = $vals
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !exclude.contains(&(i + 1)))
                    .map(|(_, v)| v.clone())
                    .collect();
                Ok(RValue::Vector(Vector::$variant(result)))
            }};
        }
        match v {
            Vector::Double(vals) => filter_vec!(vals, Double),
            Vector::Integer(vals) => filter_vec!(vals, Integer),
            Vector::Logical(vals) => filter_vec!(vals, Logical),
            Vector::Character(vals) => filter_vec!(vals, Character),
        }
    }

    fn index_by_logical(&self, v: &Vector, mask: &[Option<bool>]) -> Result<RValue, RError> {
        macro_rules! mask_vec {
            ($vals:expr, $variant:ident) => {{
                let result: Vec<_> = $vals
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask.get(*i).copied().flatten().unwrap_or(false))
                    .map(|(_, v)| v.clone())
                    .collect();
                Ok(RValue::Vector(Vector::$variant(result)))
            }};
        }
        match v {
            Vector::Double(vals) => mask_vec!(vals, Double),
            Vector::Integer(vals) => mask_vec!(vals, Integer),
            Vector::Logical(vals) => mask_vec!(vals, Logical),
            Vector::Character(vals) => mask_vec!(vals, Character),
        }
    }

    fn eval_index_double(
        &mut self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RError> {
        let obj = self.eval_in(object, env)?;
        if indices.is_empty() {
            return Ok(obj);
        }

        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
        } else {
            return Ok(obj);
        };

        match &obj {
            RValue::List(list) => match &idx_val {
                RValue::Vector(Vector::Character(names)) => {
                    if let Some(Some(name)) = names.first() {
                        for (n, v) in &list.values {
                            if n.as_ref() == Some(name) {
                                return Ok(v.clone());
                            }
                        }
                    }
                    Ok(RValue::Null)
                }
                RValue::Vector(v) => {
                    let i = v.as_integer_scalar().unwrap_or(0) as usize;
                    if i > 0 && i <= list.values.len() {
                        Ok(list.values[i - 1].1.clone())
                    } else {
                        Ok(RValue::Null)
                    }
                }
                _ => Ok(RValue::Null),
            },
            RValue::Vector(v) => {
                let i = match &idx_val {
                    RValue::Vector(iv) => iv.as_integer_scalar().unwrap_or(0) as usize,
                    _ => 0,
                };
                if i > 0 && i <= v.len() {
                    let idx = i - 1;
                    match v {
                        Vector::Double(vals) => Ok(RValue::Vector(Vector::Double(vec![vals[idx]]))),
                        Vector::Integer(vals) => {
                            Ok(RValue::Vector(Vector::Integer(vec![vals[idx]])))
                        }
                        Vector::Logical(vals) => {
                            Ok(RValue::Vector(Vector::Logical(vec![vals[idx]])))
                        }
                        Vector::Character(vals) => {
                            Ok(RValue::Vector(Vector::Character(vec![vals[idx].clone()])))
                        }
                    }
                } else {
                    Ok(RValue::Null)
                }
            }
            _ => Err(RError::Index("object is not subsettable".to_string())),
        }
    }

    fn eval_dollar(
        &mut self,
        object: &Expr,
        member: &str,
        env: &Environment,
    ) -> Result<RValue, RError> {
        let obj = self.eval_in(object, env)?;
        match &obj {
            RValue::List(list) => {
                for (name, val) in &list.values {
                    if name.as_deref() == Some(member) {
                        return Ok(val.clone());
                    }
                }
                Ok(RValue::Null)
            }
            RValue::Environment(e) => e
                .get(member)
                .ok_or_else(|| RError::Name(member.to_string())),
            _ => Ok(RValue::Null),
        }
    }

    fn eval_index_assign(
        &mut self,
        object: &Expr,
        indices: &[Arg],
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RError> {
        let var_name = match object {
            Expr::Symbol(name) => name.clone(),
            _ => return Err(RError::Other("invalid assignment target".to_string())),
        };

        let mut obj = env.get(&var_name).unwrap_or(RValue::Null);

        if indices.is_empty() {
            env.set(var_name, val.clone());
            return Ok(val);
        }

        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
        } else {
            return Ok(val);
        };

        match &mut obj {
            RValue::Vector(v) => {
                let idx_ints = match &idx_val {
                    RValue::Vector(iv) => iv.to_integers(),
                    _ => return Err(RError::Index("invalid index".to_string())),
                };

                let new_vals = match &val {
                    RValue::Vector(vv) => vv.to_doubles(),
                    _ => return Err(RError::Type("replacement value error".to_string())),
                };

                let mut doubles = v.to_doubles();
                for (j, idx) in idx_ints.iter().enumerate() {
                    if let Some(i) = idx {
                        let i = *i as usize;
                        if i > 0 {
                            // Extend if necessary
                            while doubles.len() < i {
                                doubles.push(None);
                            }
                            doubles[i - 1] = new_vals
                                .get(j % new_vals.len())
                                .copied()
                                .flatten()
                                .map(Some)
                                .unwrap_or(None);
                        }
                    }
                }
                let new_obj = RValue::Vector(Vector::Double(doubles));
                env.set(var_name, new_obj.clone());
                Ok(val)
            }
            RValue::List(list) => {
                match &idx_val {
                    RValue::Vector(Vector::Character(names)) => {
                        if let Some(Some(name)) = names.first() {
                            if let Some(entry) = list
                                .values
                                .iter_mut()
                                .find(|(n, _)| n.as_ref() == Some(name))
                            {
                                entry.1 = val.clone();
                            } else {
                                list.values.push((Some(name.clone()), val.clone()));
                            }
                        }
                    }
                    RValue::Vector(iv) => {
                        let i = iv.as_integer_scalar().unwrap_or(0) as usize;
                        if i > 0 && i <= list.values.len() {
                            list.values[i - 1].1 = val.clone();
                        }
                    }
                    _ => {}
                }
                env.set(var_name, obj);
                Ok(val)
            }
            RValue::Null => {
                // Create new vector/list
                match &idx_val {
                    RValue::Vector(Vector::Character(names)) => {
                        let mut list = RList::new(vec![]);
                        if let Some(Some(name)) = names.first() {
                            list.values.push((Some(name.clone()), val.clone()));
                        }
                        env.set(var_name, RValue::List(list));
                    }
                    _ => {
                        let idx = match &idx_val {
                            RValue::Vector(iv) => iv.as_integer_scalar().unwrap_or(0) as usize,
                            _ => 0,
                        };
                        let mut doubles = vec![None; idx];
                        if idx > 0 {
                            if let RValue::Vector(vv) = &val {
                                doubles[idx - 1] = vv.to_doubles().into_iter().next().flatten();
                            }
                        }
                        env.set(var_name, RValue::Vector(Vector::Double(doubles)));
                    }
                }
                Ok(val)
            }
            _ => Err(RError::Index("object is not subsettable".to_string())),
        }
    }

    fn eval_index_double_assign(
        &mut self,
        object: &Expr,
        indices: &[Arg],
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RError> {
        let var_name = match object {
            Expr::Symbol(name) => name.clone(),
            _ => return Err(RError::Other("invalid assignment target".to_string())),
        };

        let mut obj = env
            .get(&var_name)
            .unwrap_or(RValue::List(RList::new(vec![])));
        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
        } else {
            return Ok(val);
        };

        match &mut obj {
            RValue::List(list) => {
                match &idx_val {
                    RValue::Vector(Vector::Character(names)) => {
                        if let Some(Some(name)) = names.first() {
                            if let Some(entry) = list
                                .values
                                .iter_mut()
                                .find(|(n, _)| n.as_ref() == Some(name))
                            {
                                entry.1 = val.clone();
                            } else {
                                list.values.push((Some(name.clone()), val.clone()));
                            }
                        }
                    }
                    RValue::Vector(iv) => {
                        let i = iv.as_integer_scalar().unwrap_or(0) as usize;
                        if i > 0 {
                            while list.values.len() < i {
                                list.values.push((None, RValue::Null));
                            }
                            list.values[i - 1].1 = val.clone();
                        }
                    }
                    _ => {}
                }
                env.set(var_name, obj);
                Ok(val)
            }
            _ => self.eval_index_assign(object, indices, val, env),
        }
    }

    fn eval_dollar_assign(
        &mut self,
        object: &Expr,
        member: &str,
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RError> {
        let var_name = match object {
            Expr::Symbol(name) => name.clone(),
            _ => return Err(RError::Other("invalid assignment target".to_string())),
        };

        let mut obj = env
            .get(&var_name)
            .unwrap_or(RValue::List(RList::new(vec![])));
        match &mut obj {
            RValue::List(list) => {
                if let Some(entry) = list
                    .values
                    .iter_mut()
                    .find(|(n, _)| n.as_deref() == Some(member))
                {
                    entry.1 = val.clone();
                } else {
                    list.values.push((Some(member.to_string()), val.clone()));
                }
                env.set(var_name, obj);
                Ok(val)
            }
            RValue::Null => {
                let list = RList::new(vec![(Some(member.to_string()), val.clone())]);
                env.set(var_name, RValue::List(list));
                Ok(val)
            }
            _ => {
                // Convert to list
                let list = RList::new(vec![(Some(member.to_string()), val.clone())]);
                env.set(var_name, RValue::List(list));
                Ok(val)
            }
        }
    }

    fn eval_ns_get(
        &mut self,
        namespace: &Expr,
        name: &str,
        env: &Environment,
    ) -> Result<RValue, RError> {
        // For now, just look up the name in the global environment
        // A real implementation would use R's namespace/package system
        let _ns = self.eval_in(namespace, env)?;
        env.get(name)
            .or_else(|| self.global_env.get(name))
            .ok_or_else(|| RError::Name(format!("{}::{}", "pkg", name)))
    }

    fn eval_for(
        &mut self,
        var: &str,
        iter_val: &RValue,
        body: &Expr,
        env: &Environment,
    ) -> Result<RValue, RError> {
        match iter_val {
            RValue::Vector(v) => {
                let len = v.len();
                for i in 0..len {
                    let elem = match v {
                        Vector::Double(vals) => RValue::Vector(Vector::Double(vec![vals[i]])),
                        Vector::Integer(vals) => RValue::Vector(Vector::Integer(vec![vals[i]])),
                        Vector::Logical(vals) => RValue::Vector(Vector::Logical(vec![vals[i]])),
                        Vector::Character(vals) => {
                            RValue::Vector(Vector::Character(vec![vals[i].clone()]))
                        }
                    };
                    env.set(var.to_string(), elem);
                    match self.eval_in(body, env) {
                        Ok(_) => {}
                        Err(RError::Next) => continue,
                        Err(RError::Break) => break,
                        Err(e) => return Err(e),
                    }
                }
            }
            RValue::List(list) => {
                for (_, val) in &list.values {
                    env.set(var.to_string(), val.clone());
                    match self.eval_in(body, env) {
                        Ok(_) => {}
                        Err(RError::Next) => continue,
                        Err(RError::Break) => break,
                        Err(e) => return Err(e),
                    }
                }
            }
            RValue::Null => {}
            _ => return Err(RError::Type("invalid for() loop sequence".to_string())),
        }
        Ok(RValue::Null)
    }

    /// S3 method dispatch: look up generic.class in the environment chain
    fn dispatch_s3(
        &mut self,
        generic: &str,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
    ) -> Result<RValue, RError> {
        // Get class of first argument
        let classes = match positional.first() {
            Some(RValue::List(l)) => match l.get_attr("class") {
                Some(RValue::Vector(Vector::Character(cls))) => {
                    cls.iter().filter_map(|s| s.clone()).collect::<Vec<_>>()
                }
                _ => vec!["list".to_string()],
            },
            Some(RValue::Vector(Vector::Logical(_))) => vec!["logical".to_string()],
            Some(RValue::Vector(Vector::Integer(_))) => vec!["integer".to_string()],
            Some(RValue::Vector(Vector::Double(_))) => vec!["numeric".to_string()],
            Some(RValue::Vector(Vector::Character(_))) => vec!["character".to_string()],
            Some(RValue::Function(_)) => vec!["function".to_string()],
            Some(RValue::Null) => vec!["NULL".to_string()],
            _ => vec![],
        };

        // Try generic.class for each class in the inheritance chain
        for class in &classes {
            let method_name = format!("{}.{}", generic, class);
            if let Some(method) = env.get(&method_name) {
                return self.call_function(&method, positional, named, env);
            }
        }

        // Try generic.default
        let default_name = format!("{}.default", generic);
        if let Some(method) = env.get(&default_name) {
            return self.call_function(&method, positional, named, env);
        }

        Err(RError::Other(format!(
            "no applicable method for '{}' applied to an object of class \"{}\"",
            generic,
            classes.first().unwrap_or(&"unknown".to_string())
        )))
    }
}
