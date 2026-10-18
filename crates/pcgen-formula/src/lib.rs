//! A small, dependency-light formula language for PCGen-Rust.
//!
//! PCGen definitions such as `BONUS:HP|1+CON` and `DEFINE:FEATPOOL|1` are
//! formula expressions.  This crate implements a conventional precedence
//! parser (the same layered recursive-descent technique used by many
//! introductory compiler texts) and an evaluator that supports integers,
//! floats, booleans, strings, comparisons, and a small function library.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fmt;

/// A formula value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
}

impl Value {
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Int(value) => *value as f64,
            Value::Float(value) => *value,
            Value::Bool(value) => *value as i64 as f64,
            Value::String(_) => 0.0,
        }
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Value::Int(value) => *value,
            Value::Float(value) => *value as i64,
            Value::Bool(value) => *value as i64,
            Value::String(_) => 0,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Int(value) => *value != 0,
            Value::Float(value) => *value != 0.0,
            Value::Bool(value) => *value,
            Value::String(value) => !value.is_empty(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::String(_) => "string",
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Int(value as i64)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::String(value) => write!(f, "{value}"),
        }
    }
}

/// An expression AST.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
}

/// A parsed formula.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Formula {
    pub source: String,
    pub expr: Expr,
}

impl Formula {
    pub fn parse(source: &str) -> Result<Self, FormulaError> {
        let tokens = Lexer::new(source).tokenize()?;
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expression()?;
        parser.expect_end()?;
        Ok(Self {
            source: source.to_string(),
            expr,
        })
    }

    pub fn eval<V: VariableResolver + ?Sized>(&self, variables: &V) -> Result<Value, FormulaError> {
        eval_expr(&self.expr, variables)
    }

    pub fn eval_f64<V: VariableResolver + ?Sized>(
        &self,
        variables: &V,
    ) -> Result<f64, FormulaError> {
        Ok(self.eval(variables)?.as_f64())
    }

    pub fn eval_i64<V: VariableResolver + ?Sized>(
        &self,
        variables: &V,
    ) -> Result<i64, FormulaError> {
        Ok(self.eval(variables)?.as_i64())
    }

    /// Numeric runtime values must not silently turn strings, infinity or
    /// overflow into zero/saturated integers.
    pub fn eval_i32<V: VariableResolver + ?Sized>(
        &self,
        variables: &V,
    ) -> Result<i32, FormulaError> {
        self.eval_numeric_i32(variables, false)
    }

    pub fn eval_floor_i32<V: VariableResolver + ?Sized>(
        &self,
        variables: &V,
    ) -> Result<i32, FormulaError> {
        self.eval_numeric_i32(variables, true)
    }

    fn eval_numeric_i32<V: VariableResolver + ?Sized>(
        &self,
        variables: &V,
        floor: bool,
    ) -> Result<i32, FormulaError> {
        let value = self.eval(variables)?;
        let number = value.as_f64();
        if matches!(value, Value::String(_))
            || !number.is_finite()
            || number < i32::MIN as f64
            || number > i32::MAX as f64
        {
            return Err(FormulaError::new(
                "formula result is not a finite i32 number",
                0,
            ));
        }
        Ok(if floor { number.floor() } else { number } as i32)
    }

    pub fn variables(&self) -> BTreeSet<String> {
        let mut result = BTreeSet::new();
        collect_variables(&self.expr, &mut result);
        result
    }
}

/// Something that can resolve variable names during evaluation.
pub trait VariableResolver {
    fn resolve(&self, name: &str) -> Option<Value>;
}

impl VariableResolver for HashMap<String, Value> {
    fn resolve(&self, name: &str) -> Option<Value> {
        self.get(name).cloned()
    }
}

impl VariableResolver for HashMap<String, i64> {
    fn resolve(&self, name: &str) -> Option<Value> {
        self.get(name).copied().map(Value::Int)
    }
}

impl VariableResolver for HashMap<String, i32> {
    fn resolve(&self, name: &str) -> Option<Value> {
        self.get(name)
            .copied()
            .map(|value| Value::Int(value as i64))
    }
}

impl VariableResolver for HashMap<String, f64> {
    fn resolve(&self, name: &str) -> Option<Value> {
        self.get(name).copied().map(Value::Float)
    }
}

impl VariableResolver for HashMap<String, bool> {
    fn resolve(&self, name: &str) -> Option<Value> {
        self.get(name).copied().map(Value::Bool)
    }
}

impl VariableResolver for HashMap<String, String> {
    fn resolve(&self, name: &str) -> Option<Value> {
        self.get(name).cloned().map(Value::String)
    }
}

impl VariableResolver for () {
    fn resolve(&self, _name: &str) -> Option<Value> {
        None
    }
}

/// Convenience function for one-off expressions.
pub fn evaluate<V: VariableResolver + ?Sized>(
    source: &str,
    variables: &V,
) -> Result<Value, FormulaError> {
    Formula::parse(source)?.eval(variables)
}

/// Convenience numeric evaluation.
pub fn evaluate_f64<V: VariableResolver + ?Sized>(
    source: &str,
    variables: &V,
) -> Result<f64, FormulaError> {
    Formula::parse(source)?.eval_f64(variables)
}

/// Formula parse/evaluation error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormulaError {
    pub message: String,
    pub position: usize,
}

impl FormulaError {
    fn new(message: impl Into<String>, position: usize) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at position {}", self.message, self.position)
    }
}

impl std::error::Error for FormulaError {}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(Value),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
    Not,
}

struct Lexer<'a> {
    source: &'a [u8],
    position: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            position: 0,
        }
    }

    fn tokenize(mut self) -> Result<Vec<Token>, FormulaError> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() {
                self.position += 1;
                continue;
            }
            if ch.is_ascii_digit()
                || (ch == b'.' && self.peek_next().is_some_and(|next| next.is_ascii_digit()))
            {
                tokens.push(self.number()?);
                continue;
            }
            if ch.is_ascii_alphabetic() || ch == b'_' {
                tokens.push(Token::Ident(self.identifier()));
                continue;
            }
            let start = self.position;
            self.position += 1;
            let token = match ch {
                b'+' => Token::Plus,
                b'-' => Token::Minus,
                b'*' => {
                    if self.match_byte(b'*') {
                        Token::Caret
                    } else {
                        Token::Star
                    }
                }
                b'/' => Token::Slash,
                b'%' => Token::Percent,
                b'^' => Token::Caret,
                b'(' => Token::LParen,
                b')' => Token::RParen,
                b',' => Token::Comma,
                b'<' => {
                    if self.match_byte(b'=') {
                        Token::LessEqual
                    } else {
                        Token::Less
                    }
                }
                b'>' => {
                    if self.match_byte(b'=') {
                        Token::GreaterEqual
                    } else {
                        Token::Greater
                    }
                }
                b'=' => {
                    self.match_byte(b'=');
                    Token::Equal
                }
                b'!' => {
                    if self.match_byte(b'=') {
                        Token::NotEqual
                    } else {
                        Token::Not
                    }
                }
                b'&' => {
                    if self.match_byte(b'&') {
                        Token::And
                    } else {
                        return Err(FormulaError::new("expected '&&'", start));
                    }
                }
                b'|' => {
                    if self.match_byte(b'|') {
                        Token::Or
                    } else {
                        return Err(FormulaError::new("expected '||'", start));
                    }
                }
                _ => {
                    return Err(FormulaError::new(
                        format!("unexpected character '{}'", ch as char),
                        start,
                    ))
                }
            };
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.position).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.source.get(self.position + 1).copied()
    }

    fn match_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn number(&mut self) -> Result<Token, FormulaError> {
        let start = self.position;
        let mut is_float = false;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.position += 1;
            } else if ch == b'.' && !is_float {
                is_float = true;
                self.position += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.source[start..self.position])
            .map_err(|_| FormulaError::new("invalid number", start))?;
        if is_float {
            text.parse::<f64>()
                .map(|value| Token::Number(Value::Float(value)))
                .map_err(|_| FormulaError::new("invalid float literal", start))
        } else {
            text.parse::<i64>()
                .map(|value| Token::Number(Value::Int(value)))
                .map_err(|_| FormulaError::new("invalid integer literal", start))
        }
    }

    fn identifier(&mut self) -> String {
        let start = self.position;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == b'_' || ch == b':' {
                self.position += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.source[start..self.position]).into_owned()
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, FormulaError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_and()?;
        while self.match_token(&Token::Or) {
            let right = self.parse_and()?;
            left = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_equality()?;
        while self.match_token(&Token::And) {
            let right = self.parse_equality()?;
            left = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_relational()?;
        loop {
            let op = if self.match_token(&Token::Equal) {
                Some(BinaryOp::Equal)
            } else if self.match_token(&Token::NotEqual) {
                Some(BinaryOp::NotEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_relational()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_additive()?;
        loop {
            let op = if self.match_token(&Token::Less) {
                Some(BinaryOp::Less)
            } else if self.match_token(&Token::LessEqual) {
                Some(BinaryOp::LessEqual)
            } else if self.match_token(&Token::Greater) {
                Some(BinaryOp::Greater)
            } else if self.match_token(&Token::GreaterEqual) {
                Some(BinaryOp::GreaterEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_additive()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = if self.match_token(&Token::Plus) {
                Some(BinaryOp::Add)
            } else if self.match_token(&Token::Minus) {
                Some(BinaryOp::Subtract)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_multiplicative()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = if self.match_token(&Token::Star) {
                Some(BinaryOp::Multiply)
            } else if self.match_token(&Token::Slash) {
                Some(BinaryOp::Divide)
            } else if self.match_token(&Token::Percent) {
                Some(BinaryOp::Modulo)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, FormulaError> {
        if self.match_token(&Token::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(expr),
            });
        }
        if self.match_token(&Token::Plus) {
            return self.parse_unary();
        }
        if self.match_token(&Token::Not) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<Expr, FormulaError> {
        let left = self.parse_primary()?;
        if self.match_token(&Token::Caret) {
            let right = self.parse_unary()?;
            return Ok(Expr::Binary {
                op: BinaryOp::Power,
                left: Box::new(left),
                right: Box::new(right),
            });
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, FormulaError> {
        match self.advance() {
            Some(Token::Number(value)) => Ok(Expr::Literal(value)),
            Some(Token::Ident(name)) => {
                if self.match_token(&Token::LParen) {
                    let mut args = Vec::new();
                    if !self.match_token(&Token::RParen) {
                        loop {
                            args.push(self.parse_expression()?);
                            if self.match_token(&Token::RParen) {
                                break;
                            }
                            self.expect_token(&Token::Comma, "expected ',' or ')'")?;
                        }
                    }
                    Ok(Expr::Call { name, args })
                } else {
                    match name.to_ascii_lowercase().as_str() {
                        "true" => Ok(Expr::Literal(Value::Bool(true))),
                        "false" => Ok(Expr::Literal(Value::Bool(false))),
                        _ => Ok(Expr::Variable(name)),
                    }
                }
            }
            Some(Token::LParen) => {
                let expr = self.parse_expression()?;
                self.expect_token(&Token::RParen, "expected ')'")?;
                Ok(expr)
            }
            Some(token) => Err(self.error(format!("unexpected token {token:?}"))),
            None => Err(self.error("unexpected end of formula")),
        }
    }

    fn expect_end(&self) -> Result<(), FormulaError> {
        if self.position == self.tokens.len() {
            Ok(())
        } else {
            Err(self.error("unexpected trailing input"))
        }
    }

    fn expect_token(&mut self, token: &Token, message: &str) -> Result<(), FormulaError> {
        if self.match_token(token) {
            Ok(())
        } else {
            Err(self.error(message))
        }
    }

    fn match_token(&mut self, token: &Token) -> bool {
        if self.tokens.get(self.position) == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    fn error(&self, message: impl Into<String>) -> FormulaError {
        FormulaError::new(message, self.position)
    }
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

fn eval_expr<V: VariableResolver + ?Sized>(
    expr: &Expr,
    variables: &V,
) -> Result<Value, FormulaError> {
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Variable(name) => variables
            .resolve(name)
            .ok_or_else(|| FormulaError::new(format!("unknown variable '{name}'"), 0)),
        Expr::Unary { op, expr } => {
            let value = eval_expr(expr, variables)?;
            match op {
                UnaryOp::Plus => Ok(value),
                UnaryOp::Minus => match value {
                    Value::Int(value) => Ok(Value::Int(-value)),
                    Value::Float(value) => Ok(Value::Float(-value)),
                    other => Err(FormulaError::new(
                        format!("unary minus cannot be applied to {}", other.type_name()),
                        0,
                    )),
                },
                UnaryOp::Not => Ok(Value::Bool(!value.is_truthy())),
            }
        }
        Expr::Binary { op, left, right } => {
            if *op == BinaryOp::And {
                let left = eval_expr(left, variables)?;
                return if !left.is_truthy() {
                    Ok(Value::Bool(false))
                } else {
                    Ok(Value::Bool(eval_expr(right, variables)?.is_truthy()))
                };
            }
            if *op == BinaryOp::Or {
                let left = eval_expr(left, variables)?;
                return if left.is_truthy() {
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Bool(eval_expr(right, variables)?.is_truthy()))
                };
            }
            let left = eval_expr(left, variables)?;
            let right = eval_expr(right, variables)?;
            eval_binary(*op, left, right)
        }
        Expr::Call { name, args } => eval_call(name, args, variables),
    }
}

fn eval_call<V: VariableResolver + ?Sized>(
    name: &str,
    args: &[Expr],
    variables: &V,
) -> Result<Value, FormulaError> {
    let lowered = name.to_ascii_lowercase();
    // Only the chosen branch is a dependency of this evaluation.
    if lowered == "if" {
        if args.len() != 3 {
            return Err(FormulaError::new("if expects three arguments", 0));
        }
        let branch = if eval_expr(&args[0], variables)?.is_truthy() {
            1
        } else {
            2
        };
        return eval_expr(&args[branch], variables);
    }
    let values: Result<Vec<_>, _> = args.iter().map(|arg| eval_expr(arg, variables)).collect();
    let values = values?;
    match lowered.as_str() {
        "min" => {
            if values.iter().all(|value| matches!(value, Value::Int(_))) {
                Ok(Value::Int(
                    values.iter().map(Value::as_i64).min().unwrap_or(0),
                ))
            } else {
                Ok(Value::Float(
                    values
                        .iter()
                        .map(Value::as_f64)
                        .fold(f64::INFINITY, f64::min),
                ))
            }
        }
        "max" => {
            if values.iter().all(|value| matches!(value, Value::Int(_))) {
                Ok(Value::Int(
                    values.iter().map(Value::as_i64).max().unwrap_or(0),
                ))
            } else {
                Ok(Value::Float(
                    values
                        .iter()
                        .map(Value::as_f64)
                        .fold(f64::NEG_INFINITY, f64::max),
                ))
            }
        }
        "abs" => {
            let value = values
                .first()
                .ok_or_else(|| FormulaError::new("abs expects one argument", 0))?;
            match value {
                Value::Int(value) => Ok(Value::Int(value.abs())),
                other => Ok(Value::Float(other.as_f64().abs())),
            }
        }
        "floor" => Ok(Value::Int(
            values.first().map(Value::as_f64).unwrap_or(0.0).floor() as i64,
        )),
        "ceil" => Ok(Value::Int(
            values.first().map(Value::as_f64).unwrap_or(0.0).ceil() as i64,
        )),
        "round" => Ok(Value::Int(
            values.first().map(Value::as_f64).unwrap_or(0.0).round() as i64,
        )),
        "sqrt" => Ok(Value::Float(
            values.first().map(Value::as_f64).unwrap_or(0.0).sqrt(),
        )),
        "clamp" => {
            if values.len() != 3 {
                return Err(FormulaError::new("clamp expects three arguments", 0));
            }
            let value = values[0].as_f64();
            let low = values[1].as_f64();
            let high = values[2].as_f64();
            Ok(Value::Float(value.max(low).min(high)))
        }
        _ => Err(FormulaError::new(format!("unknown function '{name}'"), 0)),
    }
}

fn eval_binary(op: BinaryOp, left: Value, right: Value) -> Result<Value, FormulaError> {
    use BinaryOp::*;
    match op {
        Add => numeric_op(left, right, |a, b| a + b, |a, b| a + b),
        Subtract => numeric_op(left, right, |a, b| a - b, |a, b| a - b),
        Multiply => numeric_op(left, right, |a, b| a * b, |a, b| a * b),
        Divide => {
            let divisor = right.as_f64();
            if divisor == 0.0 {
                return Err(FormulaError::new("division by zero", 0));
            }
            match (&left, &right) {
                (Value::Int(a), Value::Int(b)) if *b != 0 => Ok(Value::Int(a / b)),
                _ => Ok(Value::Float(left.as_f64() / divisor)),
            }
        }
        Modulo => {
            let divisor = right.as_i64();
            if divisor == 0 {
                return Err(FormulaError::new("modulo by zero", 0));
            }
            Ok(Value::Int(left.as_i64() % divisor))
        }
        Power => Ok(Value::Float(left.as_f64().powf(right.as_f64()))),
        Less => Ok(Value::Bool(compare(&left, &right)? < 0)),
        LessEqual => Ok(Value::Bool(compare(&left, &right)? <= 0)),
        Greater => Ok(Value::Bool(compare(&left, &right)? > 0)),
        GreaterEqual => Ok(Value::Bool(compare(&left, &right)? >= 0)),
        Equal => Ok(Value::Bool(compare(&left, &right)? == 0)),
        NotEqual => Ok(Value::Bool(compare(&left, &right)? != 0)),
        And | Or => unreachable!("short-circuit operators are handled by the caller"),
    }
}

fn numeric_op(
    left: Value,
    right: Value,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> Result<Value, FormulaError> {
    match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(int_op(*a, *b))),
        (Value::Bool(_), Value::Bool(_)) => Err(FormulaError::new(
            "arithmetic on booleans is not supported",
            0,
        )),
        _ => Ok(Value::Float(float_op(left.as_f64(), right.as_f64()))),
    }
}

fn compare(left: &Value, right: &Value) -> Result<i32, FormulaError> {
    match (left, right) {
        (Value::String(a), Value::String(b)) => Ok(match a.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }),
        (Value::String(_), _) | (_, Value::String(_)) => {
            Err(FormulaError::new("cannot compare strings and numbers", 0))
        }
        (Value::Bool(a), Value::Bool(b)) => Ok(match (*a as i32).cmp(&(*b as i32)) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }),
        _ => {
            let a = left.as_f64();
            let b = right.as_f64();
            Ok(if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            })
        }
    }
}

fn collect_variables(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Literal(_) => {}
        Expr::Variable(name) => {
            out.insert(name.clone());
        }
        Expr::Unary { expr, .. } => collect_variables(expr, out),
        Expr::Binary { left, right, .. } => {
            collect_variables(left, out);
            collect_variables(right, out);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_variables(arg, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> HashMap<String, Value> {
        HashMap::from([
            ("STR".to_string(), Value::Int(16)),
            ("LEVEL".to_string(), Value::Int(4)),
            ("NAME".to_string(), Value::String("Aria".to_string())),
        ])
    }

    #[test]
    fn arithmetic_precedence() {
        assert_eq!(evaluate("1 + 2 * 3", &()).unwrap(), Value::Int(7));
        assert_eq!(evaluate("(1 + 2) * 3", &()).unwrap(), Value::Int(9));
        assert_eq!(evaluate("2 ^ 3 ^ 2", &()).unwrap(), Value::Float(512.0));
        assert_eq!(evaluate("-3 + 5", &()).unwrap(), Value::Int(2));
        assert_eq!(evaluate("10 / 3", &()).unwrap(), Value::Int(3));
        assert_eq!(evaluate("10 % 3", &()).unwrap(), Value::Int(1));
    }

    #[test]
    fn variables_and_functions() {
        let context = vars();
        assert_eq!(evaluate("STR + LEVEL", &context).unwrap(), Value::Int(20));
        assert_eq!(
            evaluate("max(STR, 4 * LEVEL)", &context).unwrap(),
            Value::Int(16)
        );
        assert_eq!(
            evaluate("if(LEVEL > 3, 10, 0)", &context).unwrap(),
            Value::Int(10)
        );
        let formula = Formula::parse("STR + LEVEL * 2").unwrap();
        assert_eq!(formula.eval_i64(&context).unwrap(), 24);
    }

    #[test]
    fn errors() {
        assert!(Formula::parse("1 +").is_err());
        assert!(evaluate("MISSING + 1", &()).is_err());
        assert!(evaluate("1 / 0", &()).is_err());
    }
}

/// Evaluate a formula with an explicit class/character scope. No source dialect
/// or engine dependency is required here; callers supply domain variable names.
pub fn evaluate_scoped_i32(
    expression: &str,
    variables: &std::collections::BTreeMap<String, i32>,
    bindings: &[(String, i32)],
) -> Result<i32, FormulaError> {
    evaluate_scoped_numeric_i32(expression, variables, bindings, false)
}

pub fn evaluate_scoped_floor_i32(
    expression: &str,
    variables: &std::collections::BTreeMap<String, i32>,
    bindings: &[(String, i32)],
) -> Result<i32, FormulaError> {
    evaluate_scoped_numeric_i32(expression, variables, bindings, true)
}

fn evaluate_scoped_numeric_i32(
    expression: &str,
    variables: &std::collections::BTreeMap<String, i32>,
    bindings: &[(String, i32)],
    floor: bool,
) -> Result<i32, FormulaError> {
    struct Scope<'a> {
        variables: &'a std::collections::BTreeMap<String, i32>,
        bindings: &'a [(String, i32)],
    }
    impl VariableResolver for Scope<'_> {
        fn resolve(&self, name: &str) -> Option<Value> {
            self.bindings
                .iter()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, value)| *value)
                .or_else(|| self.variables.get(name).copied())
                .map(Value::from)
        }
    }
    Formula::parse(expression)?.eval_numeric_i32(
        &Scope {
            variables,
            bindings,
        },
        floor,
    )
}
