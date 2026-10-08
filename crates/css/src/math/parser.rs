//! Recursive-descent parser for the `calc()` expression grammar.
//!
//! The grammar is deliberately small: sums, products, unary signs, numeric literals
//! with an optional unit or `%`, parenthesized sub-expressions, nested `calc()` (parsed
//! through), and opaque atoms for everything else. An atom is never partially consumed:
//! a construct the parser cannot read makes the whole parse fail, which keeps folding
//! from silently rewriting invalid input.

use super::expression::{MathExpression, MathValue};
use super::folding::matching_parenthesis;

/// Deepest parenthesis nesting accepted inside one value.
///
/// The parser descends one level per `(` and per nested `calc()`, so this bounds the
/// stack a single value can consume. A value that nests deeper fails to parse and is
/// left byte-for-byte as authored; [`super::folding::value_nesting_overflow`] applies
/// the same limit before parsing.
pub const MAX_VALUE_NESTING: usize = 64;

/// One cursor over an expression string.
///
/// The parser borrows its input and allocates owned units, atom text, and boxed
/// operator nodes as needed; the input may be a slice of a larger CSS string.
pub(super) struct MathParser<'a> {
    /// The expression text, without the surrounding `calc(` and `)`.
    input: &'a str,
    /// Byte offset of the next unread character.
    cursor: usize,
    /// Nesting levels entered by this parser and by the parsers that called it.
    depth: usize,
}

impl<'a> MathParser<'a> {
    /// Creates a parser positioned at the start of `input`.
    pub(super) fn new(input: &'a str) -> Self {
        Self {
            input,
            cursor: 0,
            depth: 0,
        }
    }

    /// Creates a parser for a nested `calc()` inside a parse that is already `depth`
    /// levels deep, so the nesting limit covers the value rather than one parser call.
    fn nested(input: &'a str, depth: usize) -> Self {
        Self {
            input,
            cursor: 0,
            depth,
        }
    }

    /// Enters one nesting level; `false` means the value nests past [`MAX_VALUE_NESTING`].
    fn enter(&mut self) -> bool {
        if self.depth >= MAX_VALUE_NESTING {
            return false;
        }
        self.depth += 1;
        true
    }

    /// Leaves the level entered by [`Self::enter`].
    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Parses the whole input, returning `None` when anything is left over.
    ///
    /// Trailing whitespace is allowed; trailing operators, extra tokens and empty input
    /// are not.
    pub(super) fn parse(mut self) -> Option<MathExpression> {
        let expression = self.parse_sum()?;
        self.skip_whitespace();
        (self.cursor == self.input.len()).then_some(expression)
    }

    /// Parses a sum, folding each `+`/`-` step as it goes.
    ///
    /// The loop is left-associative and stops at the first character that is not a
    /// sign, leaving it for the caller.
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

    /// Parses a product, folding each `*`/`/` step as it goes.
    ///
    /// Division by the literal number one collapses to the left operand; every other
    /// division stays a `Quotient` node so printing can parenthesize it.
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

    /// Parses a leading `+`/`-` and the operand that follows.
    ///
    /// A `-` that starts `--` is not a sign: it begins a custom property name, which
    /// must stay an atom.
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

    /// Parses one atom: a parenthesized sum, a numeric literal, or an opaque token.
    ///
    /// An identifier followed by `(` is read to its matching parenthesis. A nested
    /// `calc()` is parsed recursively, while every other function is kept verbatim as
    /// an atom; an unclosed parenthesis fails the parse.
    pub(super) fn parse_atom(&mut self) -> Option<MathExpression> {
        self.skip_whitespace();
        if self.peek() == Some('(') {
            self.cursor += 1;
            if !self.enter() {
                return None;
            }
            let expression = self.parse_sum();
            self.leave();
            let expression = expression?;
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
                if !self.enter() {
                    return None;
                }
                let nested = MathParser::nested(&self.input[open + 1..close], self.depth).parse();
                self.leave();
                let expression = nested?;
                self.cursor = close + 1;
                return Some(expression);
            }
            self.cursor = close + 1;
        }
        Some(MathExpression::Atom(
            self.input[start..self.cursor].trim().to_owned(),
        ))
    }

    /// Parses a numeric literal with an optional `%` or alphabetic unit.
    ///
    /// Stops before any following operator, so `12.5px + 1` leaves the `+` for the
    /// product level. At most one decimal point is consumed.
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

    /// Advances past Unicode whitespace.
    pub(super) fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.cursor += self.peek().map_or(0, char::len_utf8);
        }
    }

    /// Returns the next character without consuming it.
    pub(super) fn peek(&self) -> Option<char> {
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

    /// One atom at a time: a parenthesized sum, a literal with its unit, an opaque
    /// token, and a nested `calc()` that is parsed through instead of kept verbatim.
    #[test]
    fn parses_one_atom_and_leaves_the_rest_of_the_input() {
        for (input, expected) in [
            ("(2 + 3)", "5"),
            ("(1 + 2) * 3", "3"),
            (".5", ".5"),
            ("12.5px", "12.5px"),
            ("25%", "25%"),
            ("var(--x)", "var(--x)"),
            ("--custom-prop", "--custom-prop"),
            ("calc(1 + 2)", "3"),
            ("CALC(1 + 2)", "3"),
            ("min(1px, 2px)", "min(1px, 2px)"),
        ] {
            let mut parser = MathParser::new(input);
            assert_eq!(
                parser.parse_atom().unwrap().serialize(0),
                expected,
                "{input}"
            );
        }
        let mut parser = MathParser::new("1px + 2px");
        assert_eq!(parser.parse_atom().unwrap().serialize(0), "1px");
        parser.skip_whitespace();
        assert_eq!(parser.peek(), Some('+'));
    }

    /// A construct the parser cannot read fails the atom instead of consuming part of
    /// it, so folding never rewrites invalid input.
    #[test]
    fn atom_rejects_unreadable_or_unclosed_input() {
        for input in [
            "", ")", "*", "+", ";", "(", "var(--x", "rgb(1,2", "calc(1 +",
        ] {
            assert!(
                MathParser::new(input).parse_atom().is_none(),
                "{input:?} must not parse"
            );
        }
    }

    /// The nesting limit counts the parsers a value enters, so it bounds both explicit
    /// parentheses and nested `calc()` calls.
    #[test]
    fn atom_nesting_is_bounded_by_the_value_limit() {
        let at_limit = format!(
            "{}1{}",
            "(".repeat(MAX_VALUE_NESTING),
            ")".repeat(MAX_VALUE_NESTING)
        );
        assert!(MathParser::new(&at_limit).parse().is_some());
        let past_limit = format!(
            "{}1{}",
            "(".repeat(MAX_VALUE_NESTING + 1),
            ")".repeat(MAX_VALUE_NESTING + 1)
        );
        assert!(MathParser::new(&past_limit).parse().is_none());
        let nested_calc = format!(
            "{}1{}",
            "calc(".repeat(MAX_VALUE_NESTING),
            ")".repeat(MAX_VALUE_NESTING)
        );
        assert!(MathParser::new(&nested_calc).parse().is_some());
        let past_nested_calc = format!(
            "{}1{}",
            "calc(".repeat(MAX_VALUE_NESTING + 1),
            ")".repeat(MAX_VALUE_NESTING + 1)
        );
        assert!(MathParser::new(&past_nested_calc).parse().is_none());
    }

    /// Products fold left to right, scale units, and keep every division except a
    /// division by the literal one.
    #[test]
    fn parses_products_and_collapses_only_division_by_one() {
        for (input, expected) in [
            ("2 * 3 * 4", "24"),
            ("2 * 3 + 4", "6"),
            ("-2 * 3", "-6"),
            ("1px * 2 * 3", "6px"),
            ("25% * 2", "50%"),
            ("0 * 1px", "0px"),
            ("1 * var(--x)", "var(--x)"),
            ("6 / 1", "6"),
            ("6 / 1.0", "6"),
            ("var(--x) / 1", "var(--x)"),
            ("6 / 2", "6 / 2"),
            ("6 / 2 / 3", "6 / 2 / 3"),
        ] {
            let mut parser = MathParser::new(input);
            assert_eq!(
                parser.parse_product().unwrap().serialize(0),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn product_stops_before_an_operator_it_does_not_own_and_rejects_a_missing_operand() {
        let mut parser = MathParser::new("2 + 3");
        assert_eq!(parser.parse_product().unwrap().serialize(0), "2");
        parser.skip_whitespace();
        assert_eq!(parser.peek(), Some('+'));
        for input in ["2 *", "2 /", "* 3", "2 * )", "2 * * 3"] {
            assert!(
                MathParser::new(input).parse_product().is_none(),
                "{input:?} must not parse"
            );
        }
    }

    /// Sums are left-associative, treat a leading `--` as a custom property name
    /// rather than a sign, and fold only values of the same kind.
    #[test]
    fn parses_sums_left_associatively_and_keeps_mixed_operands_symbolic() {
        for (input, expected) in [
            ("1 + 2 + 3", "6"),
            ("10 - 2 - 3", "5"),
            ("10 - (2 - 3)", "11"),
            ("1 - -2", "3"),
            ("2rem + 3rem", "5rem"),
            ("2rem + 3px", "2rem + 3px"),
            ("25% + 25%", "50%"),
            ("--x + 1px", "--x + 1px"),
            ("　1　+　2", "3"),
        ] {
            let mut parser = MathParser::new(input);
            assert_eq!(
                parser.parse_sum().unwrap().serialize(0),
                expected,
                "{input}"
            );
        }
        let mut parser = MathParser::new("1)");
        assert_eq!(parser.parse_sum().unwrap().serialize(0), "1");
        assert_eq!(parser.peek(), Some(')'));
        for input in ["1 +", "1 -", "+", "- -", "1 + )"] {
            assert!(
                MathParser::new(input).parse_sum().is_none(),
                "{input:?} must not parse"
            );
        }
    }

    proptest::proptest! {
        /// Integer literals round-trip through parsing and serialization.
        #[test]
        fn integer_literals_round_trip(value in -100_000i32..100_000) {
            let text = value.to_string();
            let parsed = MathParser::new(&text).parse().unwrap();
            proptest::prop_assert_eq!(parsed.serialize(0), text);
        }

        /// Folding a sum does not depend on the order of its operands.
        #[test]
        fn folded_sums_are_commutative(left in -1000i32..1000, right in -1000i32..1000) {
            let forward = MathParser::new(&format!("{left} + {right}")).parse().unwrap();
            let backward = MathParser::new(&format!("{right} + {left}")).parse().unwrap();
            proptest::prop_assert_eq!(forward.serialize(0), backward.serialize(0));
        }

        /// Parenthesis nesting is accepted up to the documented limit and rejected past it.
        #[test]
        fn parenthesis_nesting_respects_the_value_limit(depth in 0usize..80) {
            let text = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
            proptest::prop_assert_eq!(
                MathParser::new(&text).parse().is_some(),
                depth <= MAX_VALUE_NESTING
            );
        }
    }
}
