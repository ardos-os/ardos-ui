//! # RSML (RuSt Markup Language) Compiler
//!
//! A DOM-based compiler that transforms JSX-like syntax into HyprUI Rust code.
//!
//! ## Architecture Overview
//!
//! The compiler follows a traditional compiler pipeline:
//! 1. **Tokenization**: Raw RSML text → Stream of tokens
//! 2. **Parsing**: Stream of tokens → DOM tree
//! 3. **Code Generation**: DOM tree → Rust code string
//!
//! ## Example Transformation
//!
//! Input RSML:
//! ```rsml
//! <container padding_all={16} center>
//!     <text font_size={18}>Hello World!</text>
//!     <MyComponent name="test" active />
//! </container>
//! ```
//!
//! Output Rust:
/// ```rust,ignore
/// Box::new(hyprui::Container::new().padding_all(16).center()
///     .child(Box::new(hyprui::Text::new("Hello World!").font_size(18)))
///     .child(hyprui::Component::new(MyComponent, {
///         let mut props = Default::default();
///         props.name = "test";
///         props.active = true;
///         props
///     })))
/// ```
use proc_macro::TokenStream;
use std::collections::HashMap;
use std::sync::LazyLock;

// ============================================================================
// DOM DATA STRUCTURES
// ============================================================================

/// A node in the RSML DOM tree.
///
/// The DOM represents the parsed structure before code generation.
/// This allows for easy inspection, transformation, and debugging.
#[derive(Debug, Clone, PartialEq)]
enum Node {
	/// An HTML-like element: `<tag attr="value">children</tag>`
	Element(Element),
	/// Plain text content between tags: `Hello World`
	Text(String),
	/// Rust expression in braces: `{some_variable + 1}`
	Expression(String),
}

/// An RSML element with tag name, attributes, and children.
///
/// Examples:
/// - `<container />` - self-closing with no attributes
/// - `<text font_size={16}>Hello</text>` - with attributes and text content
/// - `<MyComponent prop="value">...</MyComponent>` - component with children
#[derive(Debug, Clone, PartialEq)]
struct Element {
	/// The tag name (e.g., "container", "text", "MyComponent")
	tag_name: String,
	/// All attributes on the element
	attributes: Vec<Attribute>,
	/// Child nodes (other elements, text, or expressions)
	children: Vec<Node>,
	/// Whether this is a self-closing tag like `<container />`
	self_closing: bool,
}

/// An attribute on an RSML element.
///
/// Examples:
/// - `disabled` - boolean attribute (no value)
/// - `name="John"` - string literal value
/// - `size={42}` - expression value
#[derive(Debug, Clone, PartialEq)]
struct Attribute {
	/// The attribute name
	name: String,
	/// The attribute value (None for boolean attributes)
	value: Option<AttributeValue>,
}

/// The value of an attribute.
#[derive(Debug, Clone, PartialEq)]
enum AttributeValue {
	/// String literal: `name="value"`
	String(String),
	/// Rust expression: `size={variable + 1}`
	Expression(String),
}

// ============================================================================
// TOKENIZER
// ============================================================================

/// A token in the RSML token stream.
///
/// Tokens are the atomic units that the parser works with.
/// They represent meaningful syntax elements like tags, attributes, etc.
#[derive(Debug, Clone, PartialEq)]
enum Token {
	/// Opening tag bracket: `<`
	OpenTag,
	/// Closing tag bracket: `>`
	CloseTag,
	/// Self-closing tag: `/>`
	SelfCloseTag,
	/// End tag opening: `</`
	EndOpenTag,
	/// Identifier: tag names, attribute names, etc.
	Identifier(String),
	/// String literal in quotes: `"hello"` or `'hello'`
	StringLiteral(String),
	/// Rust expression in braces: `{code here}`
	Expression(String),
	/// Equals sign for attributes: `=`
	Equals,
	/// End of input
	Eof,
}

/// Converts raw RSML text into a stream of tokens.
///
/// The tokenizer handles:
/// - Proper brace matching for expressions `{...}`
/// - String literal parsing with escape sequences
/// - JSX-style tag syntax `<`, `>`, `</`, `/>`
/// - Identifier recognition for tag and attribute names
struct Tokenizer {
	/// Input text as a vector of characters for easy indexing
	input: Vec<char>,
	/// Current position in the input
	position: usize,
	/// Current character being processed (None at EOF)
	current_char: Option<char>,
}

impl Tokenizer {
	/// Create a new tokenizer for the given input text.
	fn new(input: &str) -> Self {
		let chars: Vec<char> = input.chars().collect();
		let current_char = chars.get(0).copied();
		Self {
			input: chars,
			position: 0,
			current_char,
		}
	}

	/// Advance to the next character in the input.
	fn advance(&mut self) {
		self.position += 1;
		self.current_char = self.input.get(self.position).copied();
	}

	/// Look at the next character without advancing.
	fn peek(&self) -> Option<char> {
		self.input.get(self.position + 1).copied()
	}

	/// Skip over whitespace characters.
	fn skip_whitespace(&mut self) {
		while let Some(ch) = self.current_char {
			if ch.is_whitespace() {
				self.advance();
			} else {
				break;
			}
		}
	}

	/// Read an identifier (tag name, attribute name, etc.).
	///
	/// Identifiers can contain letters, numbers, underscores, and hyphens.
	/// Examples: `container`, `font_size`, `MyComponent`, `data-id`
	fn read_identifier(&mut self) -> String {
		let mut result = String::new();

		while let Some(ch) = self.current_char {
			if ch.is_alphanumeric() || ch == '_' || ch == '-' {
				result.push(ch);
				self.advance();
			} else {
				break;
			}
		}

		result
	}

	/// Read a string literal, handling escape sequences.
	///
	/// Supports both double and single quotes: `"hello"` or `'hello'`
	/// Handles escape sequences like `\"` and `\\`
	fn read_string_literal(&mut self) -> String {
		let quote_char = self.current_char.unwrap(); // " or '
		self.advance(); // skip opening quote

		let mut result = String::new();
		let mut escaped = false;

		while let Some(ch) = self.current_char {
			if escaped {
				result.push(ch);
				escaped = false;
			} else if ch == '\\' {
				escaped = true;
				result.push(ch);
			} else if ch == quote_char {
				self.advance(); // skip closing quote
				break;
			} else {
				result.push(ch);
			}
			self.advance();
		}

		result
	}

	/// Read a Rust expression inside braces: `{expression here}`
	///
	/// This handles proper brace matching, so expressions like `{vec![1, 2, 3]}`
	/// or `{if condition { "yes" } else { "no" }}` are parsed correctly.
	///
	/// Also handles string literals inside expressions to avoid false matches.
	fn read_expression(&mut self) -> String {
		self.advance(); // skip opening {

		let mut result = String::new();
		let mut brace_count = 1; // We're already inside one brace
		let mut in_string = false;
		let mut string_char = '"';
		let mut escaped = false;

		while let Some(ch) = self.current_char {
			if escaped {
				result.push(ch);
				escaped = false;
			} else if ch == '\\' && in_string {
				result.push(ch);
				escaped = true;
			} else if (ch == '"' || ch == '\'') && !in_string {
				// Entering a string
				in_string = true;
				string_char = ch;
				result.push(ch);
			} else if ch == string_char && in_string {
				// Exiting a string
				in_string = false;
				result.push(ch);
			} else if !in_string {
				// Only count braces when not inside a string
				if ch == '{' {
					brace_count += 1;
					result.push(ch);
				} else if ch == '}' {
					brace_count -= 1;
					if brace_count == 0 {
						self.advance(); // skip closing }
						break;
					}
					result.push(ch);
				} else {
					result.push(ch);
				}
			} else {
				result.push(ch);
			}
			self.advance();
		}

		result
	}

	/// Get the next token from the input stream.
	///
	/// This is the main tokenizer method that identifies and returns
	/// the next meaningful token in the input.
	fn next_token(&mut self) -> Token {
		loop {
			match self.current_char {
				None => return Token::Eof,

				Some(ch) if ch.is_whitespace() => {
					self.skip_whitespace();
					continue; // Skip whitespace and continue
				}

				Some('<') => {
					if self.peek() == Some('/') {
						// Closing tag: </
						self.advance(); // skip <
						self.advance(); // skip /
						return Token::EndOpenTag;
					} else {
						// Opening tag: <
						self.advance();
						return Token::OpenTag;
					}
				}

				Some('/') if self.peek() == Some('>') => {
					// Self-closing tag: />
					self.advance(); // skip /
					self.advance(); // skip >
					return Token::SelfCloseTag;
				}

				Some('>') => {
					// End of opening tag: >
					self.advance();
					return Token::CloseTag;
				}

				Some('=') => {
					// Attribute assignment: =
					self.advance();
					return Token::Equals;
				}

				Some('"') | Some('\'') => {
					// String literal
					let string_val = self.read_string_literal();
					return Token::StringLiteral(string_val);
				}

				Some('{') => {
					// Rust expression
					let expr = self.read_expression();
					return Token::Expression(expr);
				}

				Some(ch) if ch.is_alphabetic() || ch == '_' => {
					// Identifier (tag name, attribute name, etc.)
					let ident = self.read_identifier();
					return Token::Identifier(ident);
				}

				Some(_) => {
					// Unknown character - skip it
					self.advance();
					continue;
				}
			}
		}
	}
}

// ============================================================================
// PARSER
// ============================================================================

/// Converts a stream of tokens into a DOM tree.
///
/// The parser implements a recursive descent parser that recognizes
/// the RSML grammar and builds a structured DOM representation.
struct Parser {
	/// The tokenizer that provides the token stream
	tokenizer: Tokenizer,
	/// The current token being processed
	current_token: Token,
}

impl Parser {
	/// Create a new parser for the given input text.
	fn new(input: &str) -> Self {
		let mut tokenizer = Tokenizer::new(input);
		let current_token = tokenizer.next_token();
		Self {
			tokenizer,
			current_token,
		}
	}

	/// Advance to the next token.
	fn advance(&mut self) {
		self.current_token = self.tokenizer.next_token();
	}

	/// Expect a specific token and advance, or return an error.
	///
	/// This is used to enforce the grammar rules. For example,
	/// after parsing a tag name, we expect to see either attributes or `>`.
	fn expect_token(&mut self, expected: Token) -> Result<(), String> {
		if std::mem::discriminant(&self.current_token) == std::mem::discriminant(&expected) {
			self.advance();
			Ok(())
		} else {
			Err(format!(
				"Expected {:?}, found {:?}",
				expected, self.current_token
			))
		}
	}

	/// Parse attributes from the current token position.
	///
	/// Attributes have the form:
	/// - `name="value"` - string attribute
	/// - `name={expression}` - expression attribute
	/// - `name` - boolean attribute (no value)
	///
	/// Returns a vector of parsed attributes.
	fn parse_attributes(&mut self) -> Result<Vec<Attribute>, String> {
		let mut attributes = Vec::new();

		// Keep parsing attributes while we see identifiers
		while let Token::Identifier(name) = &self.current_token {
			let attr_name = name.clone();
			self.advance();

			let value = if matches!(self.current_token, Token::Equals) {
				self.advance(); // consume =

				// Parse the attribute value
				match &self.current_token {
					Token::StringLiteral(s) => {
						let val = Some(AttributeValue::String(s.clone()));
						self.advance();
						val
					}
					Token::Expression(e) => {
						let val = Some(AttributeValue::Expression(e.clone()));
						self.advance();
						val
					}
					_ => return Err("Expected string literal or expression after =".to_string()),
				}
			} else {
				// Boolean attribute (no value means true)
				None
			};

			attributes.push(Attribute {
				name: attr_name,
				value,
			});
		}

		Ok(attributes)
	}

	/// Parse an RSML element from the token stream.
	///
	/// Elements have the form:
	/// - `<tag />` - self-closing element
	/// - `<tag>children</tag>` - element with children
	/// - `<tag attr="value">children</tag>` - element with attributes and children
	///
	/// Returns the parsed element as a Node::Element.
	fn parse_element(&mut self) -> Result<Node, String> {
		self.expect_token(Token::OpenTag)?; // consume <

		// Get the tag name
		let tag_name = match &self.current_token {
			Token::Identifier(name) => name.clone(),
			_ => return Err("Expected tag name after <".to_string()),
		};
		self.advance();

		// Parse attributes
		let attributes = self.parse_attributes()?;

		// Check for self-closing tag
		let self_closing = matches!(self.current_token, Token::SelfCloseTag);

		if self_closing {
			self.advance(); // consume />
			return Ok(Node::Element(Element {
				tag_name,
				attributes,
				children: vec![],
				self_closing: true,
			}));
		}

		// Consume the closing > of the opening tag
		self.expect_token(Token::CloseTag)?; // consume >

		let mut children = Vec::new();

		// Parse children until we hit the closing tag
		while !matches!(self.current_token, Token::EndOpenTag) {
			match &self.current_token {
				Token::OpenTag => {
					// Nested element
					children.push(self.parse_element()?);
				}
				Token::Expression(expr) => {
					// Expression child: {some_expression}
					children.push(Node::Expression(expr.clone()));
					self.advance();
				}
				Token::Identifier(_) => {
					// Text content between tags
					if let Token::Identifier(text) = &self.current_token {
						children.push(Node::Text(text.clone()));
						self.advance();
					}
				}
				Token::Eof => {
					return Err(format!("Unexpected EOF while parsing <{}>", tag_name));
				}
				_ => {
					// Skip unknown tokens
					self.advance();
				}
			}
		}

		// Parse the closing tag: </tagname>
		self.expect_token(Token::EndOpenTag)?; // consume </

		// Verify the closing tag name matches the opening tag
		if let Token::Identifier(closing_name) = &self.current_token {
			if *closing_name != tag_name {
				return Err(format!(
					"Mismatched closing tag: expected </{}>, found </{}>",
					tag_name, closing_name
				));
			}
			self.advance();
		} else {
			return Err("Expected tag name in closing tag".to_string());
		}

		self.expect_token(Token::CloseTag)?; // consume >

		Ok(Node::Element(Element {
			tag_name,
			attributes,
			children,
			self_closing: false,
		}))
	}

	/// Parse the entire RSML input and return the root DOM node.
	fn parse(&mut self) -> Result<Node, String> {
		self.parse_element()
	}
}

// ============================================================================
// CODE GENERATOR
// ============================================================================

/// Generates Rust code from a DOM tree.
///
/// The code generator traverses the DOM and produces idiomatic HyprUI Rust code.
/// It handles:
/// - Built-in elements (container, text) → Element constructors
/// - Components (uppercase tags) → Component::new with props
/// - Attributes → Method calls or prop assignments
/// - Children → .child() calls or props.children vector
struct CodeGenerator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BooleanAttrKind {
	/// 0-arg flag method (e.g. `.center()`):
	/// - `flag` => `.flag()`
	/// - `flag={expr}` => `if expr { .flag() } else { identity }`
	FlagMethod,
	/// Bool-parameter toggle method (e.g. `.floating(bool)`):
	/// - `flag` => `.flag(true)`
	/// - `flag={expr}` => `.flag(expr)`
	ToggleBoolParam,
}

static BOOLEAN_ATTR_RULES: LazyLock<HashMap<(&'static str, &'static str), BooleanAttrKind>> =
	LazyLock::new(|| {
		let mut m = HashMap::new();

		// Container-only boolean flag methods (0-arg)
		m.insert(("container", "h_expand"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "w_expand"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "w_fit"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "center"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "focusable"), BooleanAttrKind::FlagMethod);
		m.insert(("container", "focus_container"), BooleanAttrKind::FlagMethod);

		// Text-only boolean flag methods (0-arg)
		m.insert(("text", "text_center"), BooleanAttrKind::FlagMethod);
		m.insert(("text", "text_right"), BooleanAttrKind::FlagMethod);
		m.insert(("text", "text_left"), BooleanAttrKind::FlagMethod);

		// Container-only bool-parameter toggle methods
		m.insert(("container", "floating"), BooleanAttrKind::ToggleBoolParam);

		m
	});



impl CodeGenerator {
	fn new() -> Self {
		Self
	}

	/// Generate Rust code for a DOM node.
	///
	/// This is the main entry point that dispatches to specific
	/// generation methods based on the node type.
	fn generate(&self, node: &Node) -> String {
		self.generate_with_box(node, true)
	}

	/// Generate Rust code for a DOM node, with option to wrap in Box::new().
	fn generate_with_box(&self, node: &Node, wrap_in_box: bool) -> String {
		let code = match node {
			Node::Element(element) => self.generate_element_inner(element),
			Node::Text(text) => format!("hyprui::Text::new(\"{}\")", text),
			Node::Expression(expr) => expr.clone(),
		};

		if wrap_in_box && matches!(node, Node::Element(_)) {
			format!("Box::new({})", code)
		} else {
			code
		}
	}

	/// Generate Rust code for an RSML element.
	///
	/// Determines whether the element is a component (uppercase) or
	/// a built-in element (lowercase) and generates appropriate code.
	fn generate_element_inner(&self, element: &Element) -> String {
		// Components start with uppercase letters
		if element.tag_name.chars().next().unwrap().is_uppercase() {
			return self.generate_component(element);
		}

		// Map RSML tag names to HyprUI types
		let element_type = match element.tag_name.as_str() {
			"container" => "hyprui::Container",
			"text" => "hyprui::Text",
			_ => &element.tag_name,
		};

		let mut code = if element.tag_name == "text" {
			// Text has special constructor: Text::new(content)
			let format_string = element
				.children
				.iter()
				.map(|child| match child {
					Node::Text(text) => text.trim().to_string(),
					Node::Expression(_) => "{}".to_string(),
					Node::Element(element) => panic!(
						"Text element cannot contain other elements, but found {:?}",
						element
					),
				})
				.collect::<Vec<String>>()
				.join(" ");
			let fmt_args = element
				.children
				.iter()
				.filter_map(|child| match child {
					Node::Text(_) => None,
					Node::Expression(expr) => Some(expr.clone()),
					Node::Element(element) => panic!(
						"Text element cannot contain other elements, but found {:?}",
						element
					),
				})
				.collect::<Vec<String>>()
				.join(", ");
			let format_call = format!("format!(\"{}\", {})", format_string, fmt_args);
			format!(
				"{}::new({})",
				element_type,
				if fmt_args.is_empty() {
					format!("\"{format_string}\"")
				} else {
					format_call
				}
			)
		} else {
			// Regular constructor: Element::new()
			format!("{}::new()", element_type)
		};

		// Convert attributes to method calls
		for attr in &element.attributes {
			match &attr.value {
				Some(AttributeValue::String(s)) => {
					// String attribute: .method("value")
					code = format!("{}.{}(\"{}\")", code, attr.name, s);
				}
				Some(AttributeValue::Expression(e)) => {
					if let Some(kind) = BOOLEAN_ATTR_RULES
						.get(&(element.tag_name.as_str(), attr.name.as_str()))
						.copied()
					{
						match kind {
							BooleanAttrKind::FlagMethod => {
								// Boolean flag method with expression: if expr { .method() } else { identity }
								code = format!(
									"if {} {{ {}.{}() }} else {{ {} }}",
									e, code, attr.name, code
								);
							}
							BooleanAttrKind::ToggleBoolParam => {
								// Bool-parameter toggle: `.method(expr)`
								code = format!("{}.{}({})", code, attr.name, e);
							}
						}
					} else {
						// Regular method with expression: .method(expr)
						code = format!("{}.{}({})", code, attr.name, e);
					}
				}
				None => {
					if let Some(kind) = BOOLEAN_ATTR_RULES
						.get(&(element.tag_name.as_str(), attr.name.as_str()))
						.copied()
					{
						match kind {
							BooleanAttrKind::FlagMethod => {
								// Boolean attribute without value: `.method()`
								code = format!("{}.{}()", code, attr.name);
							}
							BooleanAttrKind::ToggleBoolParam => {
								// HTML-like boolean attribute without value: `.method(true)`
								code = format!("{}.{}(true)", code, attr.name);
							}
						}
					} else {
						// Default: treat as 0-arg method call.
						code = format!("{}.{}()", code, attr.name);
					}
				}
			}
		}

		// Add children as .child() calls (except for text which handle children differently)
		if element.tag_name != "text" {
			for child in &element.children {
				match child {
					Node::Text(text) if text.trim().is_empty() => {
						// Skip whitespace-only text nodes
						continue;
					}
					_ => {
						let child_code = self.generate_with_box(child, false);
						code = format!("{}.child({})", code, child_code);
					}
				}
			}
		}

		code
	}

	/// Generate Rust code for a component (uppercase tag).
	///
	/// Components are generated as Component::new(ComponentName, props)
	/// where props is built using the Default::default() pattern:
	///
	/// ```rust,ignore
	/// hyprui::Component::new(MyComponent, {
	///     let mut props = Default::default();
	///     props.name = "value";
	///     props.active = true;
	///     props.children = vec![/* child elements */];
	///     props
	/// })
	/// ```
	///
	/// This allows Rust to infer the correct props type from the component function signature.
	fn generate_component(&self, element: &Element) -> String {
		let mut props_assignments = Vec::new();

		// Convert attributes to props assignments
		for attr in &element.attributes {
			let prop_assignment = match &attr.value {
				Some(AttributeValue::String(s)) => {
					// String prop: props.name = "value";
					format!("        props.{} = \"{}\".into();", attr.name, s)
				}
				Some(AttributeValue::Expression(e)) => {
					// Expression prop: props.name = expression;
					format!("        props.{} = {}.into();", attr.name, e)
				}
				None => {
					// Boolean prop: props.name = true;
					format!("        props.{} = true.into();", attr.name)
				}
			};
			props_assignments.push(prop_assignment);
		}

		// Convert children to props.children vector
		if !element.children.is_empty() {
			let mut children_code = Vec::new();
			for child in &element.children {
				match child {
					Node::Text(text) if text.trim().is_empty() => {
						// Skip whitespace-only text nodes
						continue;
					}
					_ => {
						children_code.push(self.generate_with_box(child, true));
					}
				}
			}

			if !children_code.is_empty() {
				let children_vec = children_code.join(", ");
				props_assignments.push(format!("        props.children = vec![{}];", children_vec));
			}
		}

		if props_assignments.is_empty() {
			// No props, use Default::default() directly
			format!(
				"hyprui::Component::new({}, Default::default())",
				element.tag_name
			)
		} else {
			// Build props using Default::default() pattern
			let props_block = format!(
				"{{\n        let mut props = Default::default();\n{}\n        props\n    }}",
				props_assignments.join("\n")
			);
			format!(
				"hyprui::Component::new({}, {})",
				element.tag_name, props_block
			)
		}
	}




}

// ============================================================================
// PROC MACRO
// ============================================================================

/// The `rsml!` procedural macro for writing HyprUI components with JSX-like syntax.
///
/// This macro transforms RSML (RuSt Markup Language) syntax into HyprUI Rust code.
///
/// # Example
///
/// ```rust,ignore
/// use hyprui::rsml;
///
/// let element = rsml! {
///     <container padding_all={16} center on_click={|| println!("Clicked!")}>
///         <text font_size={18}>Hello, World!</text>
///         <text>Click me!</text>
///     </container>
/// };
/// ```
///
/// The above expands to:
///
/// ```rust,ignore
/// Box::new(hyprui::Container::new().padding_all(16).center()
///     .on_click(|| println!("Clicked!"))
///     .child(Box::new(hyprui::Text::new("Hello, World!").font_size(18)))
///     .child(Box::new(hyprui::Text::new("Click me!"))))
/// ```
#[proc_macro]
pub fn rsml(input: TokenStream) -> TokenStream {
	// Convert TokenStream to string
	let input_str = input.to_string();

	// Parse using our RSML compiler pipeline
	let mut parser = Parser::new(&input_str);
	let rust_code = match parser.parse() {
		Ok(dom) => {
			let generator = CodeGenerator::new();
			generator.generate(&dom)
		}
		Err(e) => {
			return syn::Error::new(
				proc_macro2::Span::call_site(),
				format!("RSML parse error: {}", e),
			)
			.to_compile_error()
			.into();
		}
	};

	// Parse the generated Rust code back into tokens
	match rust_code.parse::<proc_macro2::TokenStream>() {
		Ok(tokens) => tokens.into(),
		Err(e) => {
			return syn::Error::new(
				proc_macro2::Span::call_site(),
				format!(
					"Generated invalid Rust code: {}. Generated code was: {}",
					e, rust_code
				),
			)
			.to_compile_error()
			.into();
		}
	}
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;
	use std::panic;

	/// Test harness that processes all RSML test files.
	///
	/// This test harness:
	/// 1. Reads all `.rsml` files from the `rsml_tests/` directory
	/// 2. Parses each file using the RSML compiler pipeline
	/// 3. Reports success/failure for each file
	/// 4. Provides a summary of results
	///
	/// Panics are caught and reported as failures to prevent one bad
	/// file from stopping the entire test suite.
	#[test]
	fn test_all_rsml_files() {
		let inputs_dir = "rsml_tests";

		// Create inputs directory if it doesn't exist
		if !std::path::Path::new(inputs_dir).exists() {
			fs::create_dir(inputs_dir).expect("Failed to create inputs directory");
			println!("Created rsml_tests/ directory. Add your test files there.");
			return;
		}

		// Read all files in inputs directory
		let entries = match fs::read_dir(inputs_dir) {
			Ok(entries) => entries,
			Err(e) => {
				panic!("Failed to read inputs directory: {}", e);
			}
		};

		let mut total_files = 0;
		let mut passed_files = 0;

		// Process each file in the directory
		for entry in entries {
			let entry = match entry {
				Ok(entry) => entry,
				Err(e) => {
					eprintln!("Error reading directory entry: {}", e);
					continue;
				}
			};

			let path = entry.path();
			if path.is_file() {
				total_files += 1;
				let filename = path.file_name().unwrap().to_string_lossy();

				print!("Testing {}: ", filename);

				// Read the RSML file
				let source = match fs::read_to_string(&path) {
					Ok(source) => source,
					Err(e) => {
						println!("FAIL (couldn't read file: {})", e);
						continue;
					}
				};

				// Parse with panic handling to prevent crashes
				let result = panic::catch_unwind(|| {
					// Run the full compiler pipeline: tokenize → parse → generate
					let mut parser = Parser::new(&source);
					match parser.parse() {
						Ok(dom) => {
							let generator = CodeGenerator::new();
							Ok(generator.generate(&dom))
						}
						Err(e) => Err(e),
					}
				});

				// Report results
				match result {
					Ok(Ok(rust_code)) => {
						println!("PASS");
						println!("  Output: {}", rust_code);
						passed_files += 1;
					}
					Ok(Err(parse_error)) => {
						println!("FAIL (parse error: {})", parse_error);
					}
					Err(_) => {
						println!("FAIL (panic during parsing)");
					}
				}
				println!(); // Empty line for readability
			}
		}

		// Print summary
		if total_files == 0 {
			println!("No files found in rsml_tests/ directory");
		} else {
			println!("Results: {}/{} files passed", passed_files, total_files);
			if passed_files != total_files {
				panic!("Some RSML test files failed!");
			}
		}
	}

	#[test]
	fn test_debug_expression_handling() {
		// Test expression handling specifically
		let rsml_input = r#"<text>{format!("Count: {}", count)}</text>"#;

		let mut parser = Parser::new(rsml_input);
		match parser.parse() {
			Ok(dom) => {
				let generator = CodeGenerator::new();
				let rust_code = generator.generate(&dom);
				println!("Expression test - Generated code: {}", rust_code);
			}
			Err(e) => {
				println!("Expression test - Parse error: {}", e);
			}
		}
	}
}
