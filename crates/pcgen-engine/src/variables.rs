//! Character-local variable program: a variable is its highest DEFINE plus
//! stacked BONUS:VAR contributions. Dependencies request the complete value,
//! never a partially accumulated value. Each node is evaluated once per run.
use crate::effects::{EffectState, TypedBonus};
use pcgen_formula::{Formula, Value, VariableResolver};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct VariableProgram {
    nodes: BTreeMap<String, Node>,
}

struct Expression {
    text: String,
    source: Option<String>,
}

#[derive(Default)]
struct Node {
    definitions: Vec<Expression>,
    initial: Option<i32>,
    assigned: Option<i32>,
    bonuses: Vec<(Expression, Option<String>, Option<String>)>,
}

impl VariableProgram {
    pub fn define(&mut self, name: &str, text: String, source: Option<String>) {
        self.nodes
            .entry(name.into())
            .or_default()
            .definitions
            .push(Expression { text, source });
    }

    pub fn declare(&mut self, name: &str, initial: i32) {
        self.nodes
            .entry(name.into())
            .or_default()
            .initial
            .get_or_insert(initial);
    }

    pub fn set(&mut self, name: &str, value: i32) {
        self.nodes.entry(name.into()).or_default().assigned = Some(value);
    }

    pub fn bonus(
        &mut self,
        name: &str,
        text: String,
        source: Option<String>,
        bonus_type: Option<String>,
        stack_key: Option<String>,
    ) {
        self.nodes.entry(name.into()).or_default().bonuses.push((
            Expression { text, source },
            bonus_type,
            stack_key,
        ));
    }

    pub fn evaluate(&self, state: &mut EffectState) {
        let evaluator = Evaluator {
            program: self,
            ambient: state,
            values: RefCell::new(BTreeMap::new()),
            path: RefCell::new(Vec::new()),
            diagnostics: RefCell::new(Vec::new()),
        };
        for name in self.nodes.keys() {
            evaluator.value(name);
        }
        let values = evaluator.values.into_inner();
        let diagnostics = evaluator.diagnostics.into_inner();
        for (name, value) in values {
            state.variables.remove(&name);
            state.unresolved_variables.remove(&name);
            if let Some(value) = value {
                state.variables.insert(name, value);
            } else {
                state.unresolved_variables.insert(name);
            }
        }
        state.diagnostics.extend(diagnostics);
    }
}

struct Evaluator<'a> {
    program: &'a VariableProgram,
    ambient: &'a EffectState,
    values: RefCell<BTreeMap<String, Option<i32>>>,
    path: RefCell<Vec<String>>,
    diagnostics: RefCell<Vec<String>>,
}

impl Evaluator<'_> {
    fn expression(&self, name: &str, expression: &Expression) -> Option<i32> {
        let normalized = crate::effects::bind_choices(
            &expression.text,
            self.ambient,
            expression.source.as_deref(),
        );
        match Formula::parse(&normalized).and_then(|formula| formula.eval_i32(self)) {
            Ok(value) => Some(value),
            Err(error) => {
                self.diagnostics.borrow_mut().push(format!(
                    "variable `{name}` formula `{}` (source {:?}) failed: {}",
                    expression.text, expression.source, error.message
                ));
                None
            }
        }
    }

    fn value(&self, name: &str) -> Option<i32> {
        if let Some(value) = self.values.borrow().get(name) {
            return *value;
        }
        let node = self.program.nodes.get(name)?;
        if let Some(start) = self.path.borrow().iter().position(|entry| entry == name) {
            let mut cycle = self.path.borrow()[start..].to_vec();
            cycle.push(name.to_string());
            self.diagnostics
                .borrow_mut()
                .push(format!("variable dependency cycle: {}", cycle.join(" -> ")));
            return None;
        }
        self.path.borrow_mut().push(name.to_string());
        let value = self.evaluate_node(name, node);
        self.path.borrow_mut().pop();
        self.values.borrow_mut().insert(name.into(), value);
        value
    }

    fn evaluate_node(&self, name: &str, node: &Node) -> Option<i32> {
        let is_undefined =
            node.definitions.is_empty() && node.initial.is_none() && node.assigned.is_none();
        let base = if let Some(value) = node.assigned {
            value
        } else {
            let mut best = node.initial;
            for expression in &node.definitions {
                let value = self.expression(name, expression)?;
                best = Some(best.map_or(value, |old| old.max(value)));
            }
            best.unwrap_or(0)
        };
        let mut bonuses = Vec::new();
        for (expression, bonus_type, stack_key) in &node.bonuses {
            let value = if is_undefined {
                let normalized = crate::effects::bind_choices(
                    &expression.text,
                    self.ambient,
                    expression.source.as_deref(),
                );
                match Formula::parse(&normalized).and_then(|formula| formula.eval_i32(self)) {
                    Ok(val) => val,
                    Err(_) => continue,
                }
            } else {
                self.expression(name, expression)?
            };
            bonuses.push(TypedBonus {
                target: name.into(),
                value,
                bonus_type: bonus_type.clone(),
                stack_key: stack_key.clone(),
            });
        }
        let total =
            crate::bonuses::stacked_components(&bonuses, |bonus| vec![bonus.target.clone()]);
        match base.checked_add(total.get(name).copied().unwrap_or(0)) {
            Some(value) => Some(value),
            None => {
                self.diagnostics
                    .borrow_mut()
                    .push(format!("variable `{name}` overflow"));
                None
            }
        }
    }
}

impl VariableResolver for Evaluator<'_> {
    fn resolve(&self, name: &str) -> Option<Value> {
        if self.program.nodes.contains_key(name) {
            self.value(name).map(Value::from)
        } else {
            self.ambient.resolve(name)
        }
    }
}
