use super::math_expression::{MathExpression, MathValue};
use super::math_folding::matching_parenthesis;

pub(super) struct MathParser<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> MathParser<'a> {
    pub(super) fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    pub(super) fn parse(mut self) -> Option<MathExpression> {
        let expression = self.parse_sum()?;
        self.skip_whitespace();
        (self.cursor == self.input.len()).then_some(expression)
    }

    pub(super) fn parse_sum(&mut self) -> Option<MathExpression> {
        let mut expression = self.parse_product()?;
        loop {
            self.skip_whitespace();
            let Some(operator) = self.peek() else {
                break;
            };
            if operator != '+' && operator != '-' {
                break;
            }
            self.cursor += operator.len_utf8();
            let right = self.parse_product()?;
            expression = if operator == '-' {
                MathExpression::sum(expression, right.negate())
            } else {
                MathExpression::sum(expression, right)
            };
        }
        Some(expression)
    }

    pub(super) fn parse_product(&mut self) -> Option<MathExpression> {
        let mut expression = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            let Some(operator) = self.peek() else {
                break;
            };
            if operator != '*' && operator != '/' {
                break;
            }
            self.cursor += operator.len_utf8();
            let right = self.parse_unary()?;
            expression = if operator == '*' {
                MathExpression::product(expression, right)
            } else {
                match (&expression, &right) {
                    (_, MathExpression::Value(MathValue::Number(1.0))) => expression,
                    _ => MathExpression::Quotient(Box::new(expression), Box::new(right)),
                }
            };
        }
        Some(expression)
    }

    pub(super) fn parse_unary(&mut self) -> Option<MathExpression> {
        self.skip_whitespace();
        if self.peek() == Some('-') && !self.input[self.cursor..].starts_with("--") {
            self.cursor += 1;
            return Some(self.parse_unary()?.negate());
        }
        if self.peek() == Some('+') {
            self.cursor += 1;
            return self.parse_unary();
        }
        self.parse_atom()
    }

    pub(super) fn parse_atom(&mut self) -> Option<MathExpression> {
        self.skip_whitespace();
        if self.peek() == Some('(') {
            self.cursor += 1;
            let expression = self.parse_sum()?;
            self.skip_whitespace();
            (self.peek() == Some(')')).then_some(())?;
            self.cursor += 1;
            return Some(expression);
        }
        if self
            .peek()
            .is_some_and(|character| character.is_ascii_digit() || character == '.')
        {
            return self.parse_numeric();
        }
        let start = self.cursor;
        while self.peek().is_some_and(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
        }) {
            self.cursor += self.peek()?.len_utf8();
        }
        if self.cursor == start {
            return None;
        }
        if self.peek() == Some('(') {
            let open = self.cursor;
            let close = matching_parenthesis(self.input, open)?;
            if self.input[start..open].eq_ignore_ascii_case("calc") {
                let expression = MathParser::new(&self.input[open + 1..close]).parse()?;
                self.cursor = close + 1;
                return Some(expression);
            }
            self.cursor = close + 1;
        }
        Some(MathExpression::Atom(
            self.input[start..self.cursor].trim().to_owned(),
        ))
    }

    pub(super) fn parse_numeric(&mut self) -> Option<MathExpression> {
        let start = self.cursor;
        let mut decimal = false;
        while let Some(character) = self.peek() {
            if character.is_ascii_digit() {
                self.cursor += 1;
            } else if character == '.' && !decimal {
                decimal = true;
                self.cursor += 1;
            } else {
                break;
            }
        }
        let number = self.input[start..self.cursor].parse::<f64>().ok()?;
        if self.peek() == Some('%') {
            self.cursor += 1;
            return Some(MathExpression::Value(MathValue::Percentage(number)));
        }
        let unit_start = self.cursor;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            self.cursor += 1;
        }
        if unit_start == self.cursor {
            Some(MathExpression::Value(MathValue::Number(number)))
        } else {
            Some(MathExpression::Value(MathValue::Dimension(
                number,
                self.input[unit_start..self.cursor].to_owned(),
            )))
        }
    }

    pub(super) fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.cursor += self.peek().map_or(0, char::len_utf8);
        }
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.input[self.cursor..].chars().next()
    }
}
