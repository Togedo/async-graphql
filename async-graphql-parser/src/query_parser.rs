use crate::pos::Positioned;
use crate::query::*;
use crate::value::Value;
use crate::Pos;
use arrayvec::ArrayVec;
use pest::error::LineColLocation;
use pest::iterators::Pair;
use pest::Parser;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::iter::Peekable;
use std::ops::Deref;
use std::str::Chars;

#[derive(Parser)]
#[grammar = "query.pest"]
struct QueryParser;

/// Parser error
#[derive(Error, Debug, PartialEq)]
pub struct Error {
    pub pos: Pos,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<pest::error::Error<Rule>> for Error {
    fn from(err: pest::error::Error<Rule>) -> Self {
        Error {
            pos: {
                let (line, column) = match err.line_col {
                    LineColLocation::Pos((line, column)) => (line, column),
                    LineColLocation::Span((line, column), _) => (line, column),
                };
                Pos { line, column }
            },
            message: err.to_string(),
        }
    }
}

/// Parser result
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) struct PositionCalculator<'a> {
    input: Peekable<Chars<'a>>,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> PositionCalculator<'a> {
    fn new(input: &'a str) -> PositionCalculator<'a> {
        Self {
            input: input.chars().peekable(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn step(&mut self, pair: &Pair<Rule>) -> Pos {
        let pos = pair.as_span().start();
        debug_assert!(pos >= self.pos);
        for _ in 0..pos - self.pos {
            match self.input.next() {
                Some('\r') => {
                    if let Some(&'\n') = self.input.peek() {
                        self.input.next();
                        self.line += 1;
                        self.column = 1;
                    } else {
                        self.column += 1;
                    }
                }
                Some('\n') => {
                    self.line += 1;
                    self.column = 1;
                }
                Some(_) => {
                    self.column += 1;
                }
                None => break,
            }
        }
        self.pos = pos;
        Pos {
            line: self.line,
            column: self.column,
        }
    }
}

/// Parse a GraphQL query.
pub fn parse_query<T: Into<String>>(input: T) -> Result<Document> {
    let source = input.into();
    let document_pair: Pair<Rule> = QueryParser::parse(Rule::document, &source)?.next().unwrap();
    let mut definitions = Vec::new();
    let mut pc = PositionCalculator::new(&source);

    for pair in document_pair.into_inner() {
        match pair.as_rule() {
            Rule::named_operation_definition => definitions
                .push(parse_named_operation_definition(pair, &mut pc)?.pack(Definition::Operation)),
            Rule::selection_set => definitions.push(
                parse_selection_set(pair, &mut pc)?
                    .pack(OperationDefinition::SelectionSet)
                    .pack(Definition::Operation),
            ),
            Rule::fragment_definition => definitions
                .push(parse_fragment_definition(pair, &mut pc)?.pack(Definition::Fragment)),
            Rule::EOI => {}
            _ => unreachable!(),
        }
    }

    Ok(Document {
        source,
        definitions,
        fragments: Default::default(),
        current_operation: None,
    })
}

pub struct ParsedValue {
    #[allow(dead_code)]
    source: String,
    value: Value,
}

impl Deref for ParsedValue {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// Parse a graphql value
pub fn parse_value<T: Into<String>>(input: T) -> Result<ParsedValue> {
    let source = input.into();
    let value_pair: Pair<Rule> = QueryParser::parse(Rule::value, &source)?.next().unwrap();
    let mut pc = PositionCalculator::new(&source);
    let value = parse_value2(value_pair, &mut pc)?;
    Ok(ParsedValue { source, value })
}

fn parse_named_operation_definition(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<OperationDefinition>> {
    enum OperationType {
        Query,
        Mutation,
        Subscription,
    }

    let pos = pc.step(&pair);
    let mut operation_type = OperationType::Query;
    let mut name = None;
    let mut variable_definitions = None;
    let mut directives = None;
    let mut selection_set = None;

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::operation_type => {
                operation_type = match pair.as_str() {
                    "query" => OperationType::Query,
                    "mutation" => OperationType::Mutation,
                    "subscription" => OperationType::Subscription,
                    _ => unreachable!(),
                };
            }
            Rule::name => {
                name = Some(Positioned::new(
                    to_static_str(pair.as_str()),
                    pc.step(&pair),
                ));
            }
            Rule::variable_definitions => {
                variable_definitions = Some(parse_variable_definitions(pair, pc)?);
            }
            Rule::directives => {
                directives = Some(parse_directives(pair, pc)?);
            }
            Rule::selection_set => {
                selection_set = Some(parse_selection_set(pair, pc)?);
            }
            _ => unreachable!(),
        }
    }

    Ok(match operation_type {
        OperationType::Query => Positioned::new(
            Query {
                name,
                variable_definitions: variable_definitions.unwrap_or_default(),
                directives: directives.unwrap_or_default(),
                selection_set: selection_set.unwrap(),
            },
            pos,
        )
        .pack(OperationDefinition::Query),
        OperationType::Mutation => Positioned::new(
            Mutation {
                name,
                variable_definitions: variable_definitions.unwrap_or_default(),
                directives: directives.unwrap_or_default(),
                selection_set: selection_set.unwrap(),
            },
            pos,
        )
        .pack(OperationDefinition::Mutation),
        OperationType::Subscription => Positioned::new(
            Subscription {
                name,
                variable_definitions: variable_definitions.unwrap_or_default(),
                directives: directives.unwrap_or_default(),
                selection_set: selection_set.unwrap(),
            },
            pos,
        )
        .pack(OperationDefinition::Subscription),
    })
}

fn parse_default_value(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Value> {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::value => return Ok(parse_value2(pair, pc)?),
            _ => unreachable!(),
        }
    }
    unreachable!()
}

fn parse_type(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Type> {
    let pair = pair.into_inner().next().unwrap();
    match pair.as_rule() {
        Rule::nonnull_type => Ok(Type::NonNull(Box::new(parse_type(pair, pc)?))),
        Rule::list_type => Ok(Type::List(Box::new(parse_type(pair, pc)?))),
        Rule::name => Ok(Type::Named(to_static_str(pair.as_str()))),
        Rule::type_ => parse_type(pair, pc),
        _ => unreachable!(),
    }
}

fn parse_variable_definition(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<VariableDefinition>> {
    let pos = pc.step(&pair);
    let mut variable = None;
    let mut ty = None;
    let mut default_value = None;

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::variable => variable = Some(parse_variable(pair, pc)?),
            Rule::type_ => {
                ty = {
                    let pos = pc.step(&pair);
                    Some(Positioned::new(parse_type(pair, pc)?, pos))
                }
            }
            Rule::default_value => {
                let pos = pc.step(&pair);
                default_value = Some(Positioned::new(parse_default_value(pair, pc)?, pos))
            }
            _ => unreachable!(),
        }
    }
    Ok(Positioned::new(
        VariableDefinition {
            name: variable.unwrap(),
            var_type: ty.unwrap(),
            default_value,
        },
        pos,
    ))
}

fn parse_variable_definitions(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Vec<Positioned<VariableDefinition>>> {
    let mut vars = Vec::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::variable_definition => vars.push(parse_variable_definition(pair, pc)?),
            _ => unreachable!(),
        }
    }
    Ok(vars)
}

fn parse_directive(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Directive>> {
    let pos = pc.step(&pair);
    let mut name = None;
    let mut arguments = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::name => {
                let pos = pc.step(&pair);
                name = Some(Positioned::new(
                    to_static_str(to_static_str(pair.as_str())),
                    pos,
                ))
            }
            Rule::arguments => arguments = Some(parse_arguments(pair, pc)?),
            _ => unreachable!(),
        }
    }
    Ok(Positioned::new(
        Directive {
            name: name.unwrap(),
            arguments: arguments.unwrap_or_default(),
        },
        pos,
    ))
}

fn parse_directives(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Vec<Positioned<Directive>>> {
    let mut directives = Vec::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::directive => directives.push(parse_directive(pair, pc)?),
            _ => unreachable!(),
        }
    }
    Ok(directives)
}

fn parse_variable(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<&'static str>> {
    for pair in pair.into_inner() {
        if let Rule::name = pair.as_rule() {
            return Ok(Positioned::new(
                to_static_str(pair.as_str()),
                pc.step(&pair),
            ));
        }
    }
    unreachable!()
}

fn parse_value2(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Value> {
    let pair = pair.into_inner().next().unwrap();
    Ok(match pair.as_rule() {
        Rule::object => parse_object_value(pair, pc)?,
        Rule::array => parse_array_value(pair, pc)?,
        Rule::variable => Value::Variable(parse_variable(pair, pc)?.into_inner()),
        Rule::float => Value::Float(pair.as_str().parse().unwrap()),
        Rule::int => Value::Int(pair.as_str().parse().unwrap()),
        Rule::string => Value::String({
            let start_pos = pair.as_span().start_pos().line_col();
            unquote_string(
                to_static_str(pair.as_str()),
                Pos {
                    line: start_pos.0,
                    column: start_pos.1,
                },
            )?
        }),
        Rule::name => Value::Enum(to_static_str(pair.as_str())),
        Rule::boolean => Value::Boolean(match pair.as_str() {
            "true" => true,
            "false" => false,
            _ => unreachable!(),
        }),
        Rule::null => Value::Null,
        _ => unreachable!(),
    })
}

fn parse_object_pair(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<(Cow<'static, str>, Value)> {
    let mut name = None;
    let mut value = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::name => name = Some(Cow::Borrowed(to_static_str(pair.as_str()))),
            Rule::value => value = Some(parse_value2(pair, pc)?),
            _ => unreachable!(),
        }
    }
    Ok((name.unwrap(), value.unwrap()))
}

fn parse_object_value(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Value> {
    let mut map = BTreeMap::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::pair => {
                map.extend(std::iter::once(parse_object_pair(pair, pc)?));
            }
            _ => unreachable!(),
        }
    }
    Ok(Value::Object(map))
}

fn parse_array_value(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Value> {
    let mut array = Vec::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::value => {
                array.push(parse_value2(pair, pc)?);
            }
            _ => unreachable!(),
        }
    }
    Ok(Value::List(array))
}

fn parse_pair(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<(Positioned<&'static str>, Positioned<Value>)> {
    let mut name = None;
    let mut value = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::name => {
                name = Some(Positioned::new(
                    to_static_str(pair.as_str()),
                    pc.step(&pair),
                ))
            }
            Rule::value => {
                value = {
                    let pos = pc.step(&pair);
                    Some(Positioned::new(parse_value2(pair, pc)?, pos))
                }
            }
            _ => unreachable!(),
        }
    }
    Ok((name.unwrap(), value.unwrap()))
}

fn parse_arguments(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Vec<(Positioned<&'static str>, Positioned<Value>)>> {
    let mut arguments = Vec::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::pair => arguments.extend(std::iter::once(parse_pair(pair, pc)?)),
            _ => unreachable!(),
        }
    }
    Ok(arguments)
}

fn parse_alias(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<&'static str>> {
    for pair in pair.into_inner() {
        if let Rule::name = pair.as_rule() {
            return Ok(Positioned::new(
                to_static_str(pair.as_str()),
                pc.step(&pair),
            ));
        }
    }
    unreachable!()
}

fn parse_field(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Field>> {
    let pos = pc.step(&pair);
    let mut alias = None;
    let mut name = None;
    let mut directives = None;
    let mut arguments = None;
    let mut selection_set = None;

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::alias => alias = Some(parse_alias(pair, pc)?),
            Rule::name => {
                name = Some(Positioned::new(
                    to_static_str(pair.as_str()),
                    pc.step(&pair),
                ))
            }
            Rule::arguments => arguments = Some(parse_arguments(pair, pc)?),
            Rule::directives => directives = Some(parse_directives(pair, pc)?),
            Rule::selection_set => selection_set = Some(parse_selection_set(pair, pc)?),
            _ => unreachable!(),
        }
    }

    Ok(Positioned::new(
        Field {
            alias,
            name: name.unwrap(),
            arguments: arguments.unwrap_or_default(),
            directives: directives.unwrap_or_default(),
            selection_set: selection_set.unwrap_or_default(),
        },
        pos,
    ))
}

fn parse_fragment_spread(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<FragmentSpread>> {
    let pos = pc.step(&pair);
    let mut name = None;
    let mut directives = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::name => {
                name = Some(Positioned::new(
                    to_static_str(pair.as_str()),
                    pc.step(&pair),
                ))
            }
            Rule::directives => directives = Some(parse_directives(pair, pc)?),
            _ => unreachable!(),
        }
    }
    Ok(Positioned::new(
        FragmentSpread {
            fragment_name: name.unwrap(),
            directives: directives.unwrap_or_default(),
        },
        pos,
    ))
}

fn parse_type_condition(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<TypeCondition>> {
    for pair in pair.into_inner() {
        if let Rule::name = pair.as_rule() {
            let pos = pc.step(&pair);
            return Ok(Positioned::new(
                TypeCondition::On(Positioned::new(
                    to_static_str(pair.as_str()),
                    pc.step(&pair),
                )),
                pos,
            ));
        }
    }
    unreachable!()
}

fn parse_inline_fragment(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<InlineFragment>> {
    let pos = pc.step(&pair);
    let mut type_condition = None;
    let mut directives = None;
    let mut selection_set = None;

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::type_condition => type_condition = Some(parse_type_condition(pair, pc)?),
            Rule::directives => directives = Some(parse_directives(pair, pc)?),
            Rule::selection_set => selection_set = Some(parse_selection_set(pair, pc)?),
            _ => unreachable!(),
        }
    }

    Ok(Positioned::new(
        InlineFragment {
            type_condition,
            directives: directives.unwrap_or_default(),
            selection_set: selection_set.unwrap(),
        },
        pos,
    ))
}

fn parse_selection_set(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<SelectionSet>> {
    let pos = pc.step(&pair);
    let mut items = Vec::new();
    for pair in pair.into_inner().map(|pair| pair.into_inner()).flatten() {
        match pair.as_rule() {
            Rule::field => items.push(parse_field(pair, pc)?.pack(Selection::Field)),
            Rule::fragment_spread => {
                items.push(parse_fragment_spread(pair, pc)?.pack(Selection::FragmentSpread))
            }
            Rule::inline_fragment => {
                items.push(parse_inline_fragment(pair, pc)?.pack(Selection::InlineFragment))
            }
            _ => unreachable!(),
        }
    }
    Ok(Positioned::new(SelectionSet { items }, pos))
}

fn parse_fragment_definition(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<FragmentDefinition>> {
    let pos = pc.step(&pair);
    let mut name = None;
    let mut type_condition = None;
    let mut directives = None;
    let mut selection_set = None;

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::name => {
                name = Some(Positioned::new(
                    to_static_str(pair.as_str()),
                    pc.step(&pair),
                ))
            }
            Rule::type_condition => type_condition = Some(parse_type_condition(pair, pc)?),
            Rule::directives => directives = Some(parse_directives(pair, pc)?),
            Rule::selection_set => selection_set = Some(parse_selection_set(pair, pc)?),
            _ => unreachable!(),
        }
    }

    Ok(Positioned::new(
        FragmentDefinition {
            name: name.unwrap(),
            type_condition: type_condition.unwrap(),
            directives: directives.unwrap_or_default(),
            selection_set: selection_set.unwrap(),
        },
        pos,
    ))
}

#[inline]
fn to_static_str(s: &str) -> &'static str {
    unsafe { (s as *const str).as_ref().unwrap() }
}

fn unquote_string(s: &'static str, pos: Pos) -> Result<Cow<'static, str>> {
    debug_assert!(s.starts_with('"') && s.ends_with('"'));
    let s = &s[1..s.len() - 1];

    if !s.contains('\\') {
        return Ok(Cow::Borrowed(to_static_str(s)));
    }

    let mut chars = s.chars();
    let mut res = String::with_capacity(s.len());
    let mut temp_code_point = ArrayVec::<[u8; 4]>::new();

    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                match chars.next().expect("slash cant be at the end") {
                    c @ '"' | c @ '\\' | c @ '/' => res.push(c),
                    'b' => res.push('\u{0010}'),
                    'f' => res.push('\u{000C}'),
                    'n' => res.push('\n'),
                    'r' => res.push('\r'),
                    't' => res.push('\t'),
                    'u' => {
                        temp_code_point.clear();
                        for _ in 0..4 {
                            match chars.next() {
                                Some(inner_c) if inner_c.is_digit(16) => {
                                    temp_code_point.push(inner_c as u8)
                                }
                                Some(inner_c) => {
                                    return Err(Error {
                                        pos,
                                        message: format!(
                                            "{} is not a valid unicode code point",
                                            inner_c
                                        ),
                                    });
                                }
                                None => {
                                    return Err(Error {
                                        pos,
                                        message: format!(
                                            "{} must have 4 characters after it",
                                            unsafe {
                                                std::str::from_utf8_unchecked(
                                                    temp_code_point.as_slice(),
                                                )
                                            }
                                        ),
                                    });
                                }
                            }
                        }

                        // convert our hex string into a u32, then convert that into a char
                        match u32::from_str_radix(
                            unsafe { std::str::from_utf8_unchecked(temp_code_point.as_slice()) },
                            16,
                        )
                        .map(std::char::from_u32)
                        {
                            Ok(Some(unicode_char)) => res.push(unicode_char),
                            _ => {
                                return Err(Error {
                                    pos,
                                    message: format!(
                                        "{} is not a valid unicode code point",
                                        unsafe {
                                            std::str::from_utf8_unchecked(
                                                temp_code_point.as_slice(),
                                            )
                                        }
                                    ),
                                });
                            }
                        }
                    }
                    c => {
                        return Err(Error {
                            pos,
                            message: format!("bad escaped char {:?}", c),
                        });
                    }
                }
            }
            c => res.push(c),
        }
    }

    Ok(Cow::Owned(res))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parser() {
        for entry in fs::read_dir("tests/queries").unwrap() {
            if let Ok(entry) = entry {
                QueryParser::parse(Rule::document, &fs::read_to_string(entry.path()).unwrap())
                    .unwrap();
            }
        }
    }

    #[test]
    fn test_parser_ast() {
        for entry in fs::read_dir("tests/queries").unwrap() {
            if let Ok(entry) = entry {
                parse_query(fs::read_to_string(entry.path()).unwrap()).unwrap();
            }
        }
    }
}
