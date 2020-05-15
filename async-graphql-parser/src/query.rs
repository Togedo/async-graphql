use crate::pos::Positioned;
use crate::value::Value;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum Type {
    Named(&'static str),
    List(Box<Type>),
    NonNull(Box<Type>),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Named(name) => write!(f, "{}", name),
            Type::List(ty) => write!(f, "[{}]", ty),
            Type::NonNull(ty) => write!(f, "{}!", ty),
        }
    }
}

#[derive(Debug)]
pub struct Directive {
    pub name: Positioned<&'static str>,
    pub arguments: Vec<(Positioned<&'static str>, Positioned<Value>)>,
}

impl Directive {
    pub fn get_argument(&self, name: &str) -> Option<&Positioned<Value>> {
        self.arguments
            .iter()
            .find(|item| item.0.node == name)
            .map(|item| &item.1)
    }
}

pub type FragmentsMap = HashMap<&'static str, Positioned<FragmentDefinition>>;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}

#[derive(Debug)]
pub struct CurrentOperation {
    pub ty: OperationType,
    pub variable_definitions: Vec<Positioned<VariableDefinition>>,
    pub selection_set: Positioned<SelectionSet>,
}

#[derive(Debug)]
pub struct Document {
    pub(crate) source: String,
    pub(crate) definitions: Vec<Positioned<Definition>>,
    pub(crate) fragments: FragmentsMap,
    pub(crate) current_operation: Option<CurrentOperation>,
}

impl Document {
    #[inline]
    pub fn definitions(&self) -> &[Positioned<Definition>] {
        &self.definitions
    }

    #[inline]
    pub fn fragments(&self) -> &FragmentsMap {
        &self.fragments
    }

    #[inline]
    pub fn current_operation(&self) -> &CurrentOperation {
        self.current_operation
            .as_ref()
            .expect("Must first call retain_operation")
    }

    pub fn retain_operation(&mut self, operation_name: Option<&str>) -> bool {
        let mut fragments = HashMap::new();

        for definition in self.definitions.drain(..) {
            match definition.node {
                Definition::Operation(operation_definition) if self.current_operation.is_none() => {
                    match operation_definition.node {
                        OperationDefinition::SelectionSet(s) => {
                            self.current_operation = Some(CurrentOperation {
                                ty: OperationType::Query,
                                variable_definitions: Vec::new(),
                                selection_set: s,
                            });
                        }
                        OperationDefinition::Query(query)
                            if query.name.is_none()
                                || operation_name.is_none()
                                || query.name.as_ref().map(|name| name.node)
                                    == operation_name.as_deref() =>
                        {
                            self.current_operation = Some(CurrentOperation {
                                ty: OperationType::Query,
                                variable_definitions: query.node.variable_definitions,
                                selection_set: query.node.selection_set,
                            });
                        }
                        OperationDefinition::Mutation(mutation)
                            if mutation.name.is_none()
                                || operation_name.is_none()
                                || mutation.name.as_ref().map(|name| name.node)
                                    == operation_name.as_deref() =>
                        {
                            self.current_operation = Some(CurrentOperation {
                                ty: OperationType::Mutation,
                                variable_definitions: mutation.node.variable_definitions,
                                selection_set: mutation.node.selection_set,
                            });
                        }
                        OperationDefinition::Subscription(subscription)
                            if subscription.name.is_none()
                                || operation_name.is_none()
                                || subscription.name.as_ref().map(|name| name.node)
                                    == operation_name.as_deref() =>
                        {
                            self.current_operation = Some(CurrentOperation {
                                ty: OperationType::Subscription,
                                variable_definitions: subscription.node.variable_definitions,
                                selection_set: subscription.node.selection_set,
                            });
                        }
                        _ => {}
                    }
                }
                Definition::Operation(_) => {}
                Definition::Fragment(fragment) => {
                    fragments.insert(fragment.name.node, fragment);
                }
            }
        }
        self.fragments = fragments;
        self.current_operation.is_some()
    }
}

#[derive(Debug)]
pub enum Definition {
    Operation(Positioned<OperationDefinition>),
    Fragment(Positioned<FragmentDefinition>),
}

#[derive(Debug)]
pub enum TypeCondition {
    On(Positioned<&'static str>),
}

#[derive(Debug)]
pub struct FragmentDefinition {
    pub name: Positioned<&'static str>,
    pub type_condition: Positioned<TypeCondition>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}

#[derive(Debug)]
pub enum OperationDefinition {
    SelectionSet(Positioned<SelectionSet>),
    Query(Positioned<Query>),
    Mutation(Positioned<Mutation>),
    Subscription(Positioned<Subscription>),
}

#[derive(Debug)]
pub struct Query {
    pub name: Option<Positioned<&'static str>>,
    pub variable_definitions: Vec<Positioned<VariableDefinition>>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}

#[derive(Debug)]
pub struct Mutation {
    pub name: Option<Positioned<&'static str>>,
    pub variable_definitions: Vec<Positioned<VariableDefinition>>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}

#[derive(Debug)]
pub struct Subscription {
    pub name: Option<Positioned<&'static str>>,
    pub variable_definitions: Vec<Positioned<VariableDefinition>>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}

#[derive(Debug, Default)]
pub struct SelectionSet {
    pub items: Vec<Positioned<Selection>>,
}

#[derive(Debug)]
pub struct VariableDefinition {
    pub name: Positioned<&'static str>,
    pub var_type: Positioned<Type>,
    pub default_value: Option<Positioned<Value>>,
}

#[derive(Debug)]
pub enum Selection {
    Field(Positioned<Field>),
    FragmentSpread(Positioned<FragmentSpread>),
    InlineFragment(Positioned<InlineFragment>),
}

#[derive(Debug)]
pub struct Field {
    pub alias: Option<Positioned<&'static str>>,
    pub name: Positioned<&'static str>,
    pub arguments: Vec<(Positioned<&'static str>, Positioned<Value>)>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}

impl Field {
    pub fn get_argument(&self, name: &str) -> Option<&Positioned<Value>> {
        self.arguments
            .iter()
            .find(|item| item.0.node == name)
            .map(|item| &item.1)
    }
}

#[derive(Debug)]
pub struct FragmentSpread {
    pub fragment_name: Positioned<&'static str>,
    pub directives: Vec<Positioned<Directive>>,
}

#[derive(Debug)]
pub struct InlineFragment {
    pub type_condition: Option<Positioned<TypeCondition>>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}
