use super::expression::{MathExpression, MathValue};
use super::folding::matching_parenthesis;

pub(crate) struct MathParser<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> MathParser<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    pub(crate) fn parse(mut self) -> Option<MathExpression> {
        let expression = self.parse_sum()?;
        self.skip_whitespace();
        (self.cursor == self.input.len()).then_some(expression)
    }

    pub(crate) fn parse_sum(&mut self) -> Option<MathExpression> {
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

    pub(crate) fn parse_product(&mut self) -> Option<MathExpression> {
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

    pub(crate) fn parse_unary(&mut self) -> Option<MathExpression> {
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

    pub(crate) fn parse_atom(&mut self) -> Option<MathExpression> {
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

    pub(crate) fn parse_numeric(&mut self) -> Option<MathExpression> {
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

    pub(crate) fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.cursor += self.peek().map_or(0, char::len_utf8);
        }
    }

    pub(crate) fn peek(&self) -> Option<char> {
        self.input[self.cursor..].chars().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_precedence_unary_percentages_and_rejects_partial_input() {
        for (input, expected) in [
            ("2 + 3 * 4", "14"),
            ("(2 + 3) * 4", "20"),
            ("- -2px", "2px"),
            ("25% * 2", "50%"),
            ("2rem + 3rem", "5rem"),
        ] {
            assert_eq!(
                MathParser::new(input).parse().unwrap().serialize(0),
                expected
            );
        }
        for input in ["", "1 +", "(", "1 2", "1..2"] {
            assert!(MathParser::new(input).parse().is_none(), "{input}");
        }
    }

    #[test]
    fn numeric_parser_leaves_the_next_operator_unconsumed() {
        let mut parser = MathParser::new("12.5px + 1px");
        assert_eq!(parser.parse_numeric().unwrap().serialize(0), "12.5px");
        parser.skip_whitespace();
        assert_eq!(parser.peek(), Some('+'));
    }
}
